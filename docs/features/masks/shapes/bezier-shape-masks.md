---
id: F-MSK-001
title: Mask shapes: Bézier contours and primitives
status: candidate
priority: B
cost: XL
category: masks/shapes
editors: [after-effects-26, premiere-26, resolve-21, fcp-12, vegas-pro-2026]
we_have: no
crates: [kt-model, kt-fx, kt-engine, kt-gpu, kt-otio, kt-app, kt-cache]
invariants: [4, 7, 11, 12]
ui_rules: [1, 2, 3, 5, 6, 13, 16, 17, 20]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-MSK-001 — Mask shapes: Bézier contours and primitives

## Essence

A mask is a region drawn in the frame, inside which (or outside which) an effect works or the image is visible. It is drawn in two ways: a ready-made primitive (ellipse, rectangle, polygon), which has only a few numbers — center, size, angle — and a free contour of points with Bezier handles, which follows the shape of a face, a sign or a hand. A contour has four handles on top of the shape itself: feather (how soft the edge is), expansion/contraction (move the edge outward or inward without redrawing the points), mask opacity (even fully inside the region the effect can act at 40 %) and inversion. There can be several masks on one effect, and they are combined with Boolean operations. The shape is animated: the points travel along keyframes, and "blurring a car's number plate" works while the car is moving.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| After Effects | 26.x | Mask (on the layer) + masks of effects, Mask Feather Tool, RotoBezier | THE REFERENCE. A mask is a property of the LAYER and edits its alpha. Properties: Mask Path, Mask Feather (X and Y separately), Mask Opacity, Mask Expansion (±32 000 px). Each mask has its own mode out of SEVEN: None, Add, Subtract, Intersect, Lighten, Darken, Difference; they are computed in order from top to bottom in the layer's stack. The separate Mask Feather Tool places feather points ALONG the contour — the width of the soft edge differs at different places. `rotoBezier` turns on automatic calculation of handles from the position of the points |
| Adobe Premiere | 26.x | masks inside an effect (Opacity, Lumetri, Blur…), Object Masking (beta) | The mask lives on the EFFECT, not on the clip: each effect in Effect Controls has its own set of masks; Opacity has its own. Shapes: rectangle, ellipse, free contour with the pen. Handles — Mask Feather, Mask Opacity, Mask Expansion, Inverted. A separate tracking button drives the mask frame by frame forward and backward. In the 2026 line Adobe added object and vector masks and, per Adobe's help, blend modes for combining masks with each other (see "Sources", checked against a search snippet) |
| DaVinci Resolve | 21 | Power Windows (Color) + Fusion mask nodes + Magic Mask | TWO DIFFERENT tools in one program. On the Color page — windows of five kinds (Linear, Circle, Polygon, Curve, Gradient) with softness and a tracker; a WINDOW has no Boolean mode — windows in a node are combined, inversion lives at the output of the node's key, and a complex combination is made by a separate Key Mixer node (see F-CLR-006). In Fusion — a full roto: "bezier and B‑spline shapes", planar and 2D trackers, and **feathering set on EACH point separately**. In 21, Magic Mask gained "render in place": a tracked mask is cached by a separate matte node |
| Final Cut Pro | 12.3 | Shape Mask and Draw Mask (effect masks) + Shape/Draw Mask as effects | A mask belongs to a SPECIFIC effect ("mask blend modes interact within a single effect — not across multiple effects"). Shape Mask — a primitive with a Curvature handle that smoothly morphs a rectangle into an ellipse, and a second, OUTER feather circle that is dragged with the mouse. Draw Mask — a contour whose point type is switched for the whole contour at once: Linear / Bezier / B-Spline. Handles: Feather (signed, that is, outward or inward), Falloff ("controls how “steep” the feathering is"), Fill Opacity, Invert. There are three modes — Add, Subtract, Intersect — and they are computed IN ORDER of the list from top to bottom. The points of Draw Mask are shown in the inspector as NUMBERS and keyframed like ordinary parameters |
| VEGAS Pro | 2026.0 | Bezier Masking OFX, Cookie Cutter, Pan/Crop mask, Smart Mask 2.0 | Assembled from different eras. The Bezier mask arrived as a plugin in VP16 (2018): "provides complete flexibility in creating multiple masks with simple to complex shapes"; through updates — separate tangents, in VP17 — backward tracking. The primitives — the old Cookie Cutter (a shape with Repeat X replication). The mask in Pan/Crop is edited in a SEPARATE window, not in the preview. In 2026 the neural-network Smart Mask 2.0 was added on top; it is computed on the GPU and, according to users, delivers the result as Bezier masks |
| Kinotrain | — | none | There are no masks in any form. A grep over `src/libs` and `src/apps` for `mask|matte|roto|bezier|feather|spline|polygon|contour` yields exactly three things: SIMD masks in `video_simd.rs`, `IrisShape` in transitions, and `egui::Shape` (drawing primitives). There is no geometry in the model: our only "area" is `MorphTargets::Areas` — it is TIME × TRACKS, not pixels |

## The trick behind it

**1. Boolean combination of masks lives in a DIFFERENT place in each editor, and that place decides what can be said at all; and three of AE's seven modes are not Boolean at all.** In AE the mode is a property of the mask ITSELF, and the order in the layer's stack works as the order of operations: "Each additional mask you create interacts with the masks above it in the stacking order… Mask modes only operate between masks on the same layer". FCP has three modes, which are also computed from top to bottom in the list, but they act inside ONE effect: "mask blend modes interact within a single effect—not across multiple effects". In Resolve, on the Color page, a window has no Boolean operation at all: windows are stacked, inversion is at the output of the node's key, and "intersecting two windows" is already a different NODE (Key Mixer). One and the same task of the frame — "darken everything except these two areas" — turns out to be a checkbox on the mask in one editor, a rearrangement of rows in the list in another, and a new node in a third.

The second layer of the same finding becomes visible only when there are more than three modes. Add / Subtract / Intersect are operations on the AREA, while Lighten / Darken / Difference are operations on the NUMBER: they take the maximum, the minimum and the difference of the transparency values at a point. On a mask with a hard edge and 100 % opacity, Lighten coincides with Add, and Darken with Intersect, and no difference is visible. The difference appears exactly when feathering and Mask Opacity turn the mask from an area into a HALF-TONE signal. So the three extra modes are not a richness of the menu but a direct consequence of the edge being soft; and a person whose mask is sharp will consider them duplicates all their life.

**2. Uneven feathering is a SECOND contour, not a number, and the price for it is paid during animation.** Adobe's wording is direct: "Earlier versions of After Effects enabled you to add a feathered edge to a closed mask, but the width (extent) of the feather was the same around the mask. A new Mask Feather tool… has been added to let you define points along a closed mask that should have varying widths". The same is said in Fusion in one line: "set custom feathering per point". The point is that a real object does not have the same sharpness around its perimeter — the shoulder is in focus, and the arm is blurred in motion — and one number does not describe this. But feather points live ON the contour, and the contour is animated, and from this follows a consequence absent from every list of features: the feathering must be set BEFORE you have finished tracing, because on an already-set contour it does not get distributed (quote below). Exactly here the mask stops being a set of handles in a panel and becomes a second geometry, which must also be driven through time.

**3. "Shape animation" is not keys on a shape but a CORRESPONDENCE OF POINTS, and the editors silently forbid changing their number.** Between two keys the shape flows point by point: point 3 travels to point 3. So the number of points must match, and "where the numbering starts" is a separate hidden state, which AE calls First Vertex; turn it — and on the intermediate frames the contour turns inside out, although both keys look correct. Therefore the only advice given in forums sounds like a prohibition ("You should not change the number of points"), and for the same reason AE once could do what it cannot do now: up to version eight one could draw a completely new mask on every frame (quote below). The reverse side: if the correspondence is the point's INDEX, then the shape decomposes into 2·N ordinary numeric parameters, and it needs no special animation mechanism at all. FCP did exactly this: in Draw Mask the points are shown in the inspector as numbers and keyframed like everything else. The classic pain "interpolation between different contours breaks" is not a defect but exactly this fork, chosen in favor of cheapness.

## Pros

- A mask on an EFFECT (Premiere, FCP), not on a clip, gives "the same blur with different strength in two places of the frame" without duplicating the clip: two instances of the effect, each with its own pair of masks.
- A mask as a WEIGHT of the area (Resolve, Premiere, FCP) is not consumed by the composite: the next effect in the chain does not eat it. A mask as the layer's alpha (AE) does not have this property by construction — see the quote about Colorama.
- Boolean modes make "everything except" expressible without a second contour: draw what is visible and set Subtract. Without the modes the same idea has to be expressed by a second instance of the effect with an "invert" checkbox.
- Expansion/choke lets one contour serve both the matte and its cleanup: the edge is trimmed by a number, the points are not touched, the animation stays intact.
- Feathering by points replaces the pair "hard mask + soft mask" with one object, which is tracked once rather than twice.
- A shape decomposed into numbered scalars (FCP) automatically gets all the usual keyframe machinery: trimming, copy-paste, the values table, a tracker that simply writes numbers.
- A primitive (ellipse/rectangle) is 6–8 numbers, and it fits into any parameter system without a single new entity; that is precisely why it exists even where Bezier does not.

## Cons

- Correspondence by index forbids changing the number of points in the middle of an animation; yet no editor stops you from doing it — it simply silently rebuilds the whole animation. There is no return, and the undo step is one.
- First Vertex is an invisible state that affects the picture. It is not shown in the property list and makes no difference on a static frame, so it is discovered only by an inside-out intermediate frame.
- Uneven feathering is not distributed onto keys that are already set: it is itself geometry and needs its own keys.
- A mask living in a layer's alpha (AE) is destroyed by any effect lower in the chain that touches the alpha; the symptom looks like "the mask does not feather", and the cause is in another effect (quote below).
- The order in the list of masks is the order of Boolean operations, and it is also the order of rows in the interface. Drag a mask to "tidy up" — and you have changed the picture.
- The division of tools by eras breaks a single gesture: in Vegas the Bezier mask is edited in a separate Pan/Crop window, not over the preview (quote below); in Resolve the window on the Color page and the roto node in Fusion are two different editors with different shapes and different feathering within one program.
- A neural-network mask (Smart Mask, Magic Mask) cancels neither feathering, nor expansion, nor keys — it replaces only the WAY of obtaining the first contour, and adds its own price: in Vegas 2026 that means minutes of computation on the GPU and VRAM requirements.

## What users say

The only advice that an Adobe expert gave in 2017 to the question "how to redraw a mask on every frame" is a prohibition:

> When you are editing a mask the first vertex and the number of points
> are very important. You should not change the number of points.
> — Rick Gerard, Adobe Community, 06.11.2017,
> https://community.adobe.com/t5/after-effects-discussions/how-do-you-redraw-a-mask-for-every-frame/m-p/9503263

In the same thread a year and a half later — a reminder that the prohibition did not appear at once but together with interpolation:

> After Effects prior to release 8 used to let you keyframe the first
> mask and then draw a new completely different mask every frame.
> — Clark P, there, 01.05.2019

The price of uneven feathering, stated in one line:

> If you're using the mask feather tool and not the feather property of
> the mask then I don't think there is an easy way to add the feather to
> all the keyframes..
> — Emre Anil, Creative COW, 01.02.2017,
> https://creativecow.net/forums/thread/mask-feather-tool-after-keyframing/

A classic failure of the model "mask = layer alpha": a person sees that the feathering does not work, while the culprit is a completely different effect in the chain:

> Colorama effects alpha. Try using some other form of coloring.
> — Michael Szalapski, Creative COW, 04.01.2010,
> https://creativecow.net/forums/thread/after-effects-mask-feather-wont-feather-just-expan/

More recent, 2026: an attempt to animate a Bezier mask so that it rolls into the frame and out of it crashes VEGAS Pro:

> This example had the Bézier mask rolling on from the left and rolling
> off to the right - but it would seem that having position values lower
> than 0 and higher than 1 in animation crashes Vegas Pro - report sent!
> — Selinita, Boris FX Forum, 06.06.2026,
> https://forum.borisfx.com/t/discovering-vegas-pro-fx/23937

And from the same place, from a long list of complaints about VEGAS Pro 2026 — about the location of the shape editor:

> When using masks, the Pan/Crop window where the actual editing takes
> place is separated from the Preview window.
> — Cglaa5000, Boris FX Forum, 01.08.2026,
> https://forum.borisfx.com/t/suggestions-for-boris-regarding-vegas-pro-2026-very-long-post/24911

## How it fits Kinotrain

### What already exists

There are no masks, but the mathematics of primitives with a soft edge is written in our code, mirrored on the GPU and covered by tests — under the name "transitions".

- **Shape kernels with feathering.** `front_mask(s, k, soft)` — `src/libs/kt-engine/src/transitions.rs`, line 229: "coordinate → weight 0..1 with a soft front" on `smoothstep`. On it stand `MODE_IRIS` (line 317: circle, diamond, rectangle via distance from the center, with an honest `rmax` for the frame's aspect), `MODE_WIPE` (line 310: a linear gradient in an arbitrary direction), `MODE_BARN` (line 345: a band from the center), `MODE_CLOCK` (line 332: a sector by angle) and `MODE_LUMA` (line 350: a threshold on brightness with softness). These are ready-made ellipse, diamond, rectangle, gradient and sector — exactly the set of Power Windows, only the result goes not into a weight but into `mix4(a, b, w)` between two frames.
- **GPU mirror.** `front_mask` and `trans()` — `src/libs/kt-gpu/src/shaders/composite.wgsl`, lines 73 and 128, line for line with the CPU. Conformance is held by t224 (transition kinds), t181/t182 (GPU against CPU) and t225 (parameter curves): that is, the threshold kernel with a soft edge is already proven identical on two backends in our code — the most expensive part of the work for a mask has been done.
- **A rectangular frame with zoom and pan** — `pan_frame` (`src/libs/kt-fx/src/video.rs`, line 161): a crop window in the coordinates of the SOURCE with center, size and clamping, plus `bilinear_into_scalar` (line 251) and its AVX2 twin. The geometry of a rectangular mask is the same calculation, only without resampling.
- **Keyframe machinery.** `ParamCurve` / `AutoKey { t: Tick, v: f64 }` (`src/libs/kt-model/src/project.rs`, lines 1621–1626), the key domain is the MATERIAL TICK, so trim/split/reverse do not spoil the animation (F-AUT-002). One widget: `src/apps/kt-app/src/automation.rs`. If a shape is decomposed into numbered scalars (the FCP path), it lands here WITHOUT a single new entity.
- **Alpha in the frame and its blurring.** `Frame { w, h, rgba, has_alpha }` (`src/libs/kt-fx/src/video.rs`, line 21), `blur` (line 333) averages all four channels and preserves `has_alpha` — half of the choke already exists.
- **Order of computation in our favor.** FX are computed PER LAYER, and the composite goes after (`src/libs/kt-engine/src/video_plan.rs`, execution — `apply_video_chain_pooled`, `src/libs/kt-fx/src/video.rs`, line 61). A mask at a chain slot fits here naturally and does not require a new composite stage.
- **A place for "a mask on everything below"** — the morpher (`Project::effective_clips()`, `src/libs/kt-model/src/project.rs`), but with the caveat from F-MPH-007: it appends effects to the chain of EACH target, rather than working on a finished composite.

### What is missing

- **There is no geometry in the model at all.** Neither a point, nor a contour, nor a rectangle in the frame: `MorphTargets::Areas` (line 617) is time × tracks.
- **There is no rasterizer.** There is nothing to convert a polygon or a Bezier into a 0..1 coverage buffer, neither on the CPU nor on the GPU; in `src/libs/kt-gpu/` all passes are a full-screen triangle with a per-pixel formula, and vertex geometry was never introduced into the engine.
- **There is no notion of "area weight" in the plan.** An effect takes a `Frame` and returns a `Frame`; there is no separate matte signal that lives through the chain — and the alpha is silently destroyed by two effects out of ten: `edges` and `event_grid` return `has_alpha: false` (`src/libs/kt-fx/src/video.rs`, lines 453 and 737).
- **There is not a single gesture ABOVE the frame.** The preview is a picture drawn by `painter.image` in `img_rect` (`src/apps/kt-app/src/app.rs`, around line 3310); there is no code converting a screen point into a frame pixel, and the only `ui.interact` over the preview (line 3205) is taken under `Sense::click()` and is already occupied: a single click is play/pause, a double click detaches the window (items 10-11 of R.7, implemented at the user's request).
- **There is no opacity or inversion as concepts of an effect** — we have only the slot bypass and the chain master switch `fx_off`.

### What gets in the way

- **The parameter spec can hold ONLY a number, and this is not "not done yet" but a type.** `ParamSpec { name, min, max, default, step, unit, kind }` (`src/libs/kt-model/src/project.rs`, line 2203) and the macro `params_for!` (line 2261, applied to `FxInstance` at line 2308): `param_specs()` returns **`&'static [ParamSpec]`** — a static slice of fixed length. A contour of variable length cannot be expressed in this type at all. Hence the fork, and it must be named aloud rather than smeared over: **a primitive (ellipse/rectangle with center, semi-axes, angle, feathering, expansion and opacity) is 7–8 scalars that fit the existing machinery ENTIRELY** (they are drawn by `params_grid`, animated by `ParamCurve`, go into the GPU uniform, computed once in the plan), **but a Bezier contour fits nowhere.**
- **The UI rule "a numeric parameter is drawn ONLY by `widgets::param_row` / `widgets::params_grid`" (`src/apps/kt-app/CLAUDE.md`, section "Effect windows and input fields") does not work here, and this must be admitted directly.** A contour is not a number, it has no min/max/step/unit, and it cannot be drawn as a grid row. So a THIRD kind of parameter editor is needed: handles of points over the preview. It must be introduced the same way the first two were introduced — as one shared widget for the whole program, declared in the rules, otherwise after two rounds every geometric effect will have its own, and the story of slowed/pan3d will repeat, which is exactly why the rule "curves are drawn by ONE widget `automation::draw`" appeared.
- **The project has ALREADY once thrown out "its own Vec of keys inside an effect", and it must not be repeated.** `FxInstance::CamMotion { keyframes: Vec<CamKeyframe> }` (line 1958) survives only as legacy: `migrate_legacy_cammotion` (line 1833) converts it into `Pan` + `ParamCurve`, and the comment says why — "the conversion removes a duplicating animation mechanism rather than porting it" (R.34 K2). The same was done with `SlowedKeyframe` (`migrate_legacy_slowed`, line 1736). A mask with its own list of key shapes inside `FxInstance` is exactly the mechanism that was removed twice.
- **GPU: one uniform for the whole engine, 64 bytes.** `struct Params { u0, u1, f0, f1: vec4, m: mat4x4 }` and one bind group layout for all passes (`src/libs/kt-gpu/src/shaders/common.wgsl`, lines 11–23; there are exactly three textures — T0/T1/T2, and in the composite they are occupied: acc_in, layer A, layer B). Eight generic slots are enough for an ellipse and not enough for any contour: either a storage buffer of points is needed (a new layout, that is, a revision of the very decision "one uniform for the whole engine"), or rasterization of the mask on the CPU into a texture plus a fourth binding. This is the main design obstacle, and it is architectural rather than volumetric.
- **`front_mask` cannot be reused as it is — only as mathematics.** First, its threshold is the PROGRESS of a transition, and the front deliberately runs out beyond both edges by the blur width (`front = k * (1 + soft) - soft`, line 236), so that at k=0 and k=1 no visible edge remains; for a static mask this means the feathering width shifts the boundary — correct behavior for a transition and incorrect for a mask. Second, `MODE_IRIS` is rigidly centered: there is no offset, no rotation, no separate semi-axes, and all four `p0` slots of `MODE_WIPE` are already taken (dx, dy, softness). So this is not "take the function" but "write a neighbor modeled on its pattern" — and by the kt-fx rule the scalar code, the AVX2 kernel and the WGSL are edited in ONE commit, with byte-for-byte agreement between SIMD and scalar code.
- **The path is entirely 8-bit**, and feathering is the first thing that runs into this: a soft gradient over 200 px on an 8-bit scale is steps every ~0.8 px. Banding will be visible on smooth vignettes, and this is not cured by parameters.
- **The preview is computed at a DIFFERENT size than the export** (`preview_quality`, `src/libs/kt-engine/src/player.rs`; the fraction 1/16…1/1 in `src/apps/kt-app/src/app.rs`, line 3490). So the mask coordinates MUST be NORMALIZED (0..1), not in pixels, otherwise a mask placed on a 1/4 preview will drift on export. The decision is cheap, but it must be taken BEFORE the first saved project, not after.
- **The post-FX cache key.** `fx_chain_hash` (`src/libs/kt-engine/src/video.rs`, line 153) hashes the Debug form of the chain and its geometry — the mask points will get into it by themselves, but a list of a hundred points in Debug form on every frame of every layer is no longer "cheap".
- **Format.** `KT_META_VERSION` (`src/libs/kt-otio/src/meta.rs`, line 17) is now 4 → 5.

### Which invariants it touches

- **No. 7 "one composition code" — in our favor.** A mask is a WEIGHT, and the invariant names weights explicitly as what is computed once in `frame_plan` and merely executed by the backend. While the mask lives INSIDE a layer (FX per layer, before the composite), it does not introduce a new composite stage. As soon as a mask becomes "take the alpha of the neighboring layer" (track matte) — it will introduce one, and that is already a different note.
- **No. 11 determinism.** A threshold kernel is the worst case for CPU↔GPU tolerances: a divergence in the lowest bit BEFORE the threshold gives a whole pixel of difference AFTER; the tolerance will have to be formulated by the FRACTION of diverging pixels, as was already necessary in t181/t182. Plus the kt-fx rule: the default of a new parameter = the previous behavior byte for byte, otherwise 26 golden references of t26 and t67 shift.
- **No. 4 the cache is the single source of truth about coverage**: the mask parameters enter the post-FX key; the SHOW mode of the mask (silhouette instead of the picture) must NOT enter it, otherwise toggling the view would throw away warmed frames (pattern — t66).
- **No. 12 the UI heats the system only during motion**: a point handle over the preview has no right either to allocate in the draw path or to keep repainting after release; a contour of a hundred points is drawn as a MESH, not as a hundred primitives (lesson of item 40).
- Rules of `src/apps/kt-app/CLAUDE.md`: **1** (you cannot hit a 3 px point with the mouse — the grab zone is wider than the drawing), **2** (editing a mask must work DURING PLAYBACK), **3** (the panel does not jump when a mask is selected), **5** (selecting a mask does not cancel the clip selection), **6** (feathering and expansion have a min and a max), **13** (the "show mask" switch takes an already occupied place in the preview strip, not a new row), **16** (you cannot animate a shape with a different number of points — show an ERROR, not a grey hover), **17** (the gesture over the frame and the numbers in the inspector are paired ways of one action and must behave identically), **20** "nothing grows by itself" — and here is a direct collision: adding a point to the contour in the middle of an animation rewrites ALL the keys of the shape. Either this is forbidden, or it is one explicit action with one undo step and an explicit warning.

Separately about the place of the gesture: the single and double click over the preview are ALREADY taken (play/pause and window detaching, `src/apps/kt-app/src/app.rs`, line 3205, implemented at the user's direct request in R.7). The mask editing mode must be an EXPLICIT state, not "a click on the frame", otherwise rule 5 is violated.

## Estimate

| What | Answer |
|---|---|
| Size | XL overall, but it honestly splits into two unrelated parts. **(a) A primitive** — ellipse/rectangle/gradient with feathering, expansion, opacity and inversion: **M**, because these are 7–8 scalars that fit `ParamSpec`/`ParamCurve`/`params_grid`/the GPU uniform as they are, and the kernels (`front_mask`, `MODE_IRIS`, `MODE_WIPE`) are written and mirrored. **(b) A Bezier contour** with points, handles, feathering by points and shape animation: **L…XL**, because it breaks the type `&'static [ParamSpec]`, requires a rasterizer, a third kind of parameter editor and a revision of the single GPU uniform |
| Where the main work is | kt-model (geometry in the schema, specs, commands), kt-fx (mask kernel: scalar + AVX2), kt-gpu (WGSL mirror; for a contour — rasterization and/or a new bind group layout), kt-engine (mask as weight in `frame_plan`, post-FX cache key), kt-app (a shared geometry editor over the preview, mask show mode), kt-otio (v5) |
| Project format | needs v5 (`KT_META_VERSION` 4 → 5); defaults are not serialized — old files stay byte-stable |
| Tests that will have to be written | "default == previous bytes" on GOLDEN_VIDEO, t26 and t67; CPU↔GPU conformance following the pattern of t181, but with a FRACTIONAL tolerance (threshold kernel); the mask as a function (same input → same weight) on both backends; normalized coordinates: a mask on the 1/4 preview and on the 1/1 export covers the SAME region of the frame; the mask show mode does NOT invalidate the post-FX cache (pattern t66); round-trip of the model and reading of files without the new fields; during shape animation — that the number of points is constant across all keys, and that an attempt to change it gives an error, not a silent rebuild; harness tests in `layout_tests.rs` — the point handle, hitting it on the first try, working during playback |
| Depends on | nothing: `Blur`, `Glow`, `EventGrid` (mosaic) and `Greenscreen` already exist, and the mask is needed for them TODAY. Conversely, part (b) of this note is exactly what F-CLR-003 deferred to its most expensive item, and the question "where the mask lives" is a continuation of F-CLR-006 |
| Breaks compatibility | no |

## Verdict

**We take part (a) and do not take part (b) in one piece.** The boundary runs not between "there are masks" and "there are no masks", but between SCALAR geometry and arbitrary geometry — the same boundary that was already found in F-CLR-003, and finding it a second time from the other side is a confirmation, not a coincidence. An ellipse and a rectangle with feathering, expansion, opacity and inversion are a handful of numbers; the kernels for them are written, mirrored in WGSL and covered by t224/t181/t182, and numbers we can handle in every way. Such a primitive immediately fixes real tasks for which we already have effects: blurring a face (`EventGrid`), darkening a corner (`Blur` + levels), highlighting the center (`Glow`).

The main question that the note answers, and which must be settled BEFORE the round: **where the mask lives.** There are three options, and they are not equivalent. On the CLIP — cheap, but then "two blurs of different strength in two places" is inexpressible (a minus for which Premiere users still pay). As a SEPARATE OBJECT with its own key input — that is a node graph, and it has already been rejected by an explicit decision in F-CLR-006. What remains is **at the CHAIN SLOT** — as in FCP: a mask belongs to a specific effect, several masks are combined by Boolean modes in the order of the list inside one slot, and slots do not interact with each other. This is the only option that breaks nothing: FX are already computed per layer, `chain_strip` already draws the list of slots, and the cache key already hashes the chain.

And two admissions, spoken aloud, which cost more than any size estimate. First: the rule "a numeric parameter is drawn ONLY by `param_row`/`params_grid`" does not work on a contour and cannot work — a contour is not a number; a third kind of editor is needed, introduced as a shared widget and written into the rules BEFORE its first user, otherwise the slowed/pan3d divergence will repeat. Second: shape animation runs into point correspondence by index, and it is reasonable to accept the same restriction as everyone else ("the number of points is the same on all keys"), but to accept it EXPLICITLY and show a violation as an error under rule 16 — and not the way the neighbors do it, where the program silently rebuilds the animation and says nothing at all.

## Sources

- https://helpx.adobe.com/after-effects/desktop/work-with-transparency-and-compositing/work-with-alpha-channels-and-masks/alpha-channels-masks-mattes.html — AE: seven mask modes, the order in the stack ("Each additional mask you create interacts with the masks above it in the stacking order… Mask modes only operate between masks on the same layer"), Mask Expansion ±32 000 px, and the paragraph about the Mask Feather tool ("…the width (extent) of the feather was the same around the mask… to let you define points along a closed mask that should have varying widths") (2026-09-07)
- https://github.com/pushREC/after-effects-sdk-kb — a mirror of the official AE Scripting Guide: the enumeration `MaskMode` (NONE, ADD, SUBTRACT, INTERSECT, LIGHTEN, DARKEN, DIFFERENCE) and the properties of `MaskPropertyGroup` (`maskFeatherFalloff`, `rotoBezier`, `inverted`) (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/combine-multiple-effect-masks-ver2531c0ab1/mac — FCP: three modes with verbatim descriptions, "The Add, Subtract, and Intersect modes operate sequentially, starting at the top of the mask list in the Video inspector" and "mask blend modes interact within a single effect—not across multiple effects" (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/mask-controls-verf8949b45e/mac — FCP: Shape Mask (Radius X/Y, Curvature, Fill Opacity, Invert, signed Feather, Falloff — "controls how “steep” the feathering is"); Draw Mask (Shape Type: Linear / Bezier / B-Spline; Control Points as separate inspector parameters) (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/add-a-shape-mask-to-a-video-effect-vera352bed48/mac — FCP: the outer feather circle dragged with the mouse; viewing the mask as an alpha channel (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/animate-a-mask-with-keyframes-vercef2b6821/mac — FCP: for Draw Mask "you can keyframe its control points numerically in the Video inspector" (2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/fusion — Resolve/Fusion: "advanced rotoscoping with bezier and B‑spline shapes", "set custom feathering per point for precise blending", "Masks and roto shapes can all be animated to morph…" (2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/color — Resolve: "Power Windows let you define a selection by drawing shapes around specific objects in a scene", Circle/Curve/Gradient shapes in the screenshots, the tracker (2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/whatsnew — Resolve 21: "The Magic Mask palette now features a render in place option, allowing you to cache a tracked mask as a traveling matte node" (2026-09-07)
- https://forum.borisfx.com/t/vegas-pro-release-history/24284 — VEGAS: VP16 build 248 "Bezier Masking OFX plug-in provides complete flexibility in creating multiple masks with simple to complex shapes"; VP16 Update 1 "You can now split tangents on the Bézier Masking OFX plug-in"; VP16 Update 2 "Bézier Masking Edit Mode is now properly limited to only one mask at a time"; VP17 Update 2 "Backwards tracking for Bézier Mask video effect" (2026-09-07)
- https://forum.borisfx.com/t/discovering-vegas-pro-fx/23937 — VEGAS 2026: Cookie Cutter as a way to make a mask ("by putting this before Pan/Crop it will move with the underlying event"), Repeat X for several circles, and the crash on animating a Bezier mask outside 0..1 (2026-09-07)
- https://forum.borisfx.com/t/smart-mask-2-0-questions-on-this-feature/23456 — VEGAS 2026: Smart Mask 2.0 is computed on the GPU ("does use the GPU, heavily"), a minute at 60 fps ≈ 7 minutes of computation on an RTX 2080 Ti; the thread author's question — whether the result can still be exported as Bezier masks (2026-09-07)
- https://community.adobe.com/t5/after-effects-discussions/how-do-you-redraw-a-mask-for-every-frame/m-p/9503263 — AE: first vertex and the number of points, and the historical note about the behavior before version eight (2026-09-07)
- https://creativecow.net/forums/thread/mask-feather-tool-after-keyframing/ and https://creativecow.net/forums/thread/after-effects-mask-feather-wont-feather-just-expan/ — the price of uneven feathering with keys already set; the rejection of the model "mask = layer alpha" (2026-09-07)
- Code (read, not retold by analogy):
  `src/libs/kt-engine/src/transitions.rs`,
  `src/libs/kt-gpu/src/shaders/composite.wgsl`,
  `src/libs/kt-gpu/src/shaders/common.wgsl`,
  `src/libs/kt-fx/src/video.rs`, `src/libs/kt-model/src/project.rs`,
  `src/libs/kt-engine/src/video.rs`, `src/libs/kt-otio/src/meta.rs`,
  `src/apps/kt-app/src/app.rs`, `src/apps/kt-app/src/widgets.rs`,
  `src/apps/kt-app/src/fx_registry.rs`, `src/apps/kt-app/CLAUDE.md`
- `docs/user-feedback.md` R.29 item 3 ("I would like to move in the direction of After Effects, i.e. effects should have a compositing engine") — masks lie exactly in this direction; related notes: F-CLR-003 (windows and qualifier), F-CLR-006 (where the key lives), F-AUT-002 (the key domain), F-MPH-007 (what the layer sees at input)

**On the degree of verification.** `helpx.adobe.com`, `reddit.com`, `forum.blackmagicdesign.com` and `web.archive.org` did not open directly from this environment; the AE help page was obtained through the public proxy reader `r.jina.ai`, so its quotes are verbatim, but were read not from the Adobe domain itself. The **Premiere** row is the weakest in the table: the help pages "Refine and combine masks in Premiere" and "Adjust mask properties" were served as a JS stub, and the claim that the 2026 line gained mask blend MODES (add / subtract / intersect) is taken from a search snippet of the same Adobe page plus a secondary article of 22.01.2026 — **[NOT VERIFIED]**, to be checked against the live help before the round. The structure of Power Windows in Resolve (five kinds: Linear, Circle, Polygon, Curve, Gradient; combining windows inside a node and Key Mixer for complex combinations) is taken from blackmagicdesign.com and from a secondary analysis; the PDF manual could not be obtained in the environment — the details of combining are to be marked **[NOT VERIFIED]**. None of the five editors was launched live. The statements "we have / we do not have" were checked by grep over `src/` and by reading the files, not from the brief.
