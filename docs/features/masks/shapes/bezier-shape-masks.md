---
id: F-MSK-001
title: Mask shapes: Bézier contours and primitives
status: done
priority: B
cost: XL
category: masks/shapes
editors: [after-effects-26, premiere-26, resolve-21, fcp-12, vegas-pro-2026]
we_have: yes
crates: [kt-model, kt-fx, kt-engine, kt-gpu, kt-otio, kt-app, kt-cache]
invariants: [4, 7, 11, 12]
ui_rules: [1, 2, 3, 5, 6, 13, 16, 17, 20]
round: kkt-26.11.01
point: S1
tests: [t1260, t1261, t1266, t1267, t1268, t1269, t1270, t1274, t1275, t1276, t1277, t1278]
depends_on: []
checked: 2026-11-01
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
CAPABILITY: openspec/specs/video-fx/ — the rasterizer's contract is in `spec.md` (with `video-plan` for the plan, `automation` for the path's rows and keys), the "why" and the paths in the code are in `design.md`.
Shapes are a `kt_model::Mask`; one CPU rasterizer (`kt_fx::mask`) gives the coverage, the GPU applies the same bytes; the editor over the picture is `kt-app/src/maskctl.rs`.

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

### Verdict, revised by the owner's request (2026-11-01, kkt-26.11.01 S1 — change `shape-masks`)

**Done, both parts at once — (a) and (b).** The owner asked for "a convenient mechanism of
classic masks", and the orchestrator's brief named the Bézier pen on the preview. Two things this
note called obstacles in September had gone away by then, and that is why (b) no longer needed a
separate round:

- **"`&'static [ParamSpec]` cannot hold a contour"** — the passport is built at run time since R.59.
  A mask's passport (`Mask::specs`) has a row per vertex coordinate and tangent, named by the
  vertex's stable id (`v3.x`), so a deletion never renames another point. The path animates with
  the ONE curve mechanism, keyed as a whole (AE's "Mask Path"), and an inserted vertex is keyed at
  every key so the shape at the keys does not change. This is trick 3 of this note, accepted
  explicitly: the shape interpolates by point correspondence.
- **"One 128-byte uniform, three textures — the contour fits nowhere on the GPU"** — the GPU does
  not rasterize at all. One CPU rasterizer (`kt_fx::mask`: exact-area scanline inside, a signed
  distance band for antialiasing, feather with falloff and expansion; on a coarser grid for a wide
  band) produces an 8-bit coverage once per frame per application point; the GPU uploads THE SAME
  bytes as a texture and applies them in one integer pass. A mask adds no divergence (t1274).

Modes: add, subtract, intersect, difference as the soft Boolean forms; AE's Lighten/Darken/None
were not taken (trick 1: they differ only on soft masks). Variable feather along the contour
(trick 2) is the next step — the vertex is a struct, it has room. The editor over the frame is the
R.64 machinery, one call site with the transform frame, one set of handles at a time.

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
