---
id: F-CLR-002
title: RGB/Luma curves and curves "by attribute"
status: candidate
priority: A
cost: L
category: color/primary
editors: [resolve-20, premiere-26, fcp-12, vegas-pro-2026, after-effects-26]
we_have: no
crates: [kt-model, kt-fx, kt-gpu, kt-engine, kt-otio, kt-app]
invariants: [7, 11, 12]
ui_rules: [1, 8, 9, 13, 17]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-CLR-002 — RGB/Luma curves and curves "by attribute"

## Essence

A "input → output" curve on a square. The diagonal means nothing has changed; raise the middle and the picture gets lighter; place two points, pull the left one down and raise the right one, and you get an S: shadows darker, highlights brighter, midtones almost untouched. Overall brightness (the composite curve) is adjusted separately, and each of R, G and B is adjusted separately — this is how a stray tint is removed from the shadows without touching the highlights. A second, fundamentally different group: the X axis does not show a channel value but a FEATURE of the pixel — its hue or saturation — and the curve says what to do with the pixels that have that feature. "Mute only the reds" is a single mouse movement, without a selection and without a mask.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| DaVinci Resolve | 20.3.2 | Curves: Custom + Hue Vs Hue / Hue Vs Sat / Hue Vs Lum / Lum Vs Sat / Sat Vs Sat | a palette on the Color page, with its own palette at each node. Custom — Y/R/G/B, channels can be linked and unlinked. The full set of feature-based curves, and the point is placed by CLICKING ON THE PICTURE in the viewer rather than on the graph (the vendor's own description says a value is chosen simply by clicking in the viewer). The benchmark that users of every other editor compare against |
| Adobe Premiere | 26.x | Lumetri Color → Curves: RGB Curves + Hue Saturation Curves | one section of the panel with two blocks: classic RGB/Luma, and feature-based curves with an eyedropper (Hue vs Sat is confirmed by support threads). The panel is an effect on the clip; the same Lumetri carries over into After Effects. The section has a heavy reputation: complaints about a stray tint caused by the Lumetri curves themselves, and the "the only way out is the old RGB Curves" advice, have been around since 2019 |
| Final Cut Pro | 12.3 | two SEPARATE effects: Color Curves and Hue/Saturation Curves | Color Curves — Luma, Red, Green, Blue plus a free-form curve for "custom color": pick the eyedropper at the top of one of the colour curves, then click or drag over a colour in the viewer. Hue/Saturation Curves — six curves (Hue vs Hue, Hue vs Sat, Hue vs Luma, and three by saturation and brightness ranges). These are EFFECTS that you add, not a permanent palette |
| VEGAS Pro | 2026.0 | "Color Curves" (OFX) and the Color Grading panel | TWO unconnected systems in one product. The old OFX Color Curves — composite plus R/G/B, with points that have TYPES, like animation keyframes (Linear — white-grey, Hold — red). The Color Grading panel — wheels, HSL sliders, curves, LUTs; feature-based curves arrived late (per the release history: Hue vs Hue in VP20, August 2022; Hue vs Sat, Sat vs Sat and the curve eyedropper in VP20 build 214, October 2022; Hue vs Lum and Lum vs Sat do not appear in the release history). The panel does NOT support keyframing at this point; the vendor's forum states this directly |
| After Effects | 26.x | Curves effect + Lumetri Color effect | its own Curves — channels only (RGB and each channel separately, plus alpha), a pencil for a free-form map, saving and loading a curve as a file. Feature-based curves do not come from its own effect but from the same Lumetri as in Premiere |

## The trick behind it

**1. A point on a feature-based curve is not a point but a TRIPLE, and its two outer points ARE the very qualifier.** Apple's help describes the mechanism literally: you sample a colour from the picture with the eyedropper, and three control points appear on the curve. The middle one is the colour you sampled; the two outer ones limit the selection to a narrow range of hues. So one gesture of "poking the foliage" does what, in a classic grading setup, you would build as a secondary correction: it selects a range of tone, gives it a width (softness is literally the distance to the outer points), and opens a handle for modifying it. Colourists value this not as a convenience but as a DIFFERENT QUALITY of result: they report that hue-vs-hue and hue-vs-sat curves are faster to use than secondaries and sometimes produce fewer artifacts than a qualifier. This also shows how such a curve DIFFERS from an RGB curve rather than merely "resembling" one: in an RGB curve both axes are the same quantity (input and output of one channel), and "do nothing" is the DIAGONAL; in a feature-based curve the axes are different quantities (X — hue, Y — saturation), "do nothing" is the HORIZONTAL line in the middle, and the eyedropper is mandatory, because you cannot reliably hit the right hue on a horizontal scale by eye: the graph has no picture on it.

**2. The domain of this curve is a RING, but the widget is a rectangle, and this is still broken in the reference product.** Hue is cyclic: 360° is again 0°, so the curve has to continue across the seam, and a point at red has to live at both edges at once. Resolve does not do this: users report that on hue-vs-hue/sat curves in the Color page the green input hue gets stuck when you try to shift it past zero toward yellow, and they ask for it to wrap back around to 359.99. The same class of error sits at the other end: an RGB curve's end points are pinned to (0,0) and (255,255), and you cannot move the black point to the right without adding extra points; users ask for end points that move along the X axis, as in Resolve, without the extra points and the fiddling with their tangents. And a third case of the same illness is step size: the first item in the vendor's "COLOR GRADING CURVES IMPROVEMENTS" list asks for arrow keys to move a point in steps of 256 (matching 8-bit levels) rather than 100. Three different complaints, one cause: the curve widget is drawn without regard to what domain the curve has and what quantisation the pipeline beneath it uses.

**3. Five feature-based curves look alike, but they break in different ways, and the first to break is the one that touches BRIGHTNESS.** Users report that a slight Hue-to-Lum adjustment to darken a blue sky makes the clip fall apart into small fragments and a mosaic structure, while the same strength of movement on Hue vs Hue and Hue vs Sat works fine. A colourist's answer is the same: Hue vs Lum is a very tricky correction that needs a lot of detail and a soft correction, and it gets choppy very quickly. And this is not only about 8 bits: on 12-bit BRAW, Sat vs Sat produces bands, because the gradation between the affected and unaffected areas is much harsher and creates unnatural bands within a red surface. The practical conclusion for anyone building this: the artifact comes not from bit depth but from the HARSHNESS of the selection's falloff, and the softness of the selection boundary is as mandatory a parameter as the curve itself.

## Pros

- Selection and modification are ONE object: there is nothing to fall out of sync, nothing to forget to switch off, and editing the width of the selection is done with the same points as editing its edge.
- Softness is visible as the slope of the curve, not as a number "softness" in a field: it is clear by eye whether the transition will be ragged, even before the mouse is released.
- Fewer artifacts than a qualifier on the same task — by the direct testimony of colorists, and without a second layer and without a key.
- The composite curve and the channels are one widget with one vocabulary: learn a gesture once and apply it both to brightness and to a color shift.
- In an integer 8-bit path a curve is the cheapest effect possible: a table of 256 inputs is computed once per frame, and applying it is a single array lookup per channel, cheaper than any convolution.
- A curve trivially reduces to a LUT and back, so it can be exported and applied by another tool; no other kind of correction has such an exchange.

## Cons

- Eight bits: a steep curve GLUES neighbouring levels together. "This is an issue
  with 8-bit in general: it falls apart very quickly if you try to hit it
  too hard. Skies have always been a problem". The price of the table's cheapness is exactly this.
- A hard falloff of the selection gives bands regardless of bit depth — Sat vs
  Sat bands on 12-bit BRAW. So each per-attribute curve must have its own softness control, and this is one more entity in the UI.
- The ends are pinned. One cannot clip the shadows or highlights without a separate mechanism (shifting the end along X, soft clip) — the user places extra points and fights with their tangents.
- The hue axis is closed, while the rectangle is not: an edit "across 0°" requires either honest cyclic interpolation or duplicating points on both edges. Without this, red hues are edited worse than all the others — and this is an observed defect, not a theory.
- Two color systems in one product diverge: in VEGAS the old OFX can take keyframes, the new panel cannot, and the user has to keep in mind which of the two tools they are coloring with today.
- A curve point made from a keyframe drags along someone else's properties: in VEGAS it has a Linear/Hold type, the "new key type" setting does not act on it, and it is impossible to explain this to the user, because for a color curve "hold" means nothing.
- The eyedropper binds the tool to the VIEWER: the curve window stops being self-sufficient and requires a frame at hand. And grabbing the point after the eyedropper is a separate ailment: in Lumetri, after choosing a color with the eyedropper, the point sometimes cannot be taken with the mouse.
- A per-attribute curve knows nothing about space: "desaturate the reds" desaturates them both in a face and in a sign in the background. The area can be limited only by a mask — a tool the curve does not have.

## What users say

Why people hold on to per-attribute curves — the colorist's wording, not marketing:

> First - hue vs hue/hue vs sat is faster to use than secondaries /
> Second - sometimes it will produce less artifacts then qualifier
> — Val Andreev, Lift Gamma Gain, 23.10.2014,
> https://www.liftgammagain.com/forum/index.php?threads/hue-vs-hue-as-compared-to-using-secondaries.3743/

Not all curves are equal — the one that touches brightness is the one that breaks:

> When I e.g. try to darken the blue sky by slight Hue to Lum curve
> adjustment, the clip scene will start to fall apart in small "fragments
> or mosaic structure" with huge lumination variations.
> — Jan Frojdman, Blackmagic Forum, 02.03.2015,
> https://forum.blackmagicdesign.com/viewtopic.php?t=33024

> Hue vs. Lum is a very, very tricky correction where you need lots of
> detail and soft correction in order to make it work. It gets choppy very
> quickly.
> — Marc Wielage, same forum, 03.03.2015

Bands come not only from eight bits — the hardness of the selection falloff too:

> With Sat vs Sat, the gradation between what is affected and what is not,
> seems a lot harsher. It creates unnatural bands within the red of the
> bike.
> — InterrobangMP (Paul Rayner), Blackmagic Forum, 16.08.2022 (to the question
> «is it 8 bit?» the author replied «It's BRAW»),
> https://forum.blackmagicdesign.com/viewtopic.php?t=165970

> This is an issue with 8-bit in general: it falls apart very quickly if
> you try to hit it too hard. Skies have always been a problem,
> particularly with drastic changes in luminance and saturation.
> — Marc Wielage, Blackmagic Forum, 23.10.2015,
> https://forum.blackmagicdesign.com/viewtopic.php?t=33024

The most valuable are "left because of this" and "turned it on once and never again":

> I shot Prores HQ footage and I color graded it in Vegas Pro 21, in Vegas
> I get color banding but in Premiere pro it looks perfect.
> — Jimmie Brady, Creative COW, 04.06.2024 (same thread, his "I prefer editing
> with Vegas and I would hate to switch", and the advice in reply: "maybe do your
> color grading in Premiere and re-output the results into Vegas"),
> https://creativecow.net/forums/thread/color-banding-only-in-vegas-pro-not-other-softwares/

> I tried to use the RGB Curves (obsolete) and it worked perfectly, no
> yellow tint, so I'm no longer using the Lumetri Curves.
> — Jorge Jaramillo Hdz, Adobe Support Community, ≈01.2022,
> https://community.adobe.com/t5/premiere-pro-discussions/problem-with-lumetri-curves/td-p/10422834

The curve's domain is not closed — in the reference, in 2024:

> when adjusting the green input hue on hue vs hue/sat curves on the color
> page, it gets stuck if you try to shift the input hue further toward
> yellow than 0.00.
> — shmowells, Blackmagic Forum, 12.03.2024,
> https://forum.blackmagicdesign.com/viewtopic.php?t=197921

Curve widget: the point cannot be grabbed after the eyedropper, and for a neighbour a click on the point brings the program down:

> After using the eyedropper to select a color area in Hue vs Sat in
> Lumetri Color, I am not able to adjust the saturation. Mouse selecting
> one of the control "dots" does not work.
> — gsasaki, Adobe Support Community, ≈11.2025,
> https://community.adobe.com/t5/premiere-pro-discussions/hue-vs-sat/m-p/15548381

> I am using 'Color curves' video effect. When I am adding new points they
> are still 'Linear' (white/grey points), but I want them to be red as a
> default one ('Hold' points).
> — marcin_z, VEGAS Pro forum, 22.04.2026,
> https://forum.borisfx.com/t/hold-type-of-video-effect-as-a-default-point-in-vegas/22713

And what those who actually use the curve ask for:

> Allow curve end points to be moved along the X axis (like Resolve etc.),
> without having to make extra points (and fiddle with their tangency) in
> order to achieve that.
> — NickHope, VEGAS Pro forum, 04.04.2026 (in the same list — "Allow anchor
> points to be moved up and down with the arrow keys in 256 increments
> (corresponding to 8-bit levels), rather than 100 increments"),
> https://forum.borisfx.com/t/suggestions-improvements-new-features-boris-vegas-pro-2026/21773

## How it fits Kinotrain

### What is already there

- **Nothing from colour correction.** Checked by grep for `hue|saturation|lut|gamma|colorspace|scopes|histogram|vectorscope` in `src/libs/kt-fx/src`, `src/libs/kt-gpu/src`, `src/libs/kt-model/src` — zero matches. There is no dithering either.
- **A ready 8-bit path, end to end.** `src/libs/kt-media/src/lib.rs:51` ("Video frame: RGBA8") → swscale into RGBA (`src/libs/kt-ffmpeg/src/source_impl.rs`) → the FX chain (`src/libs/kt-fx/src/video.rs`, "RGBA8 in/out") → compositing in integers (`alpha_k` 0..4096, `src/libs/kt-engine/src/video.rs:53`) → muxing RGBA→`AV_PIX_FMT_YUV420P` (`src/libs/kt-ffmpeg/src/muxer_impl.rs:242`). The GPU texture format is `Rgba8Uint` (`src/libs/kt-gpu/src/pool.rs:13`) and was chosen DELIBERATELY: `src/libs/kt-gpu/src/shaders/common.wgsl` says that "u8/255.0 in f32 loses precision and breaks the mirrors of integer CPU arithmetic".
- **The road for adding a video FX is well trodden and short.** A grep for `FxInstance::Glow` gives exactly six files: `kt-model/src/project.rs`, `kt-fx/src/video.rs`, `kt-fx/tests/video_fx.rs`, `kt-gpu/src/engine.rs`, `kt-gpu/tests/parity.rs`, `kt-app/src/fx_editor.rs`.
- **The colour eyedropper is already drawn** — `ui.color_edit_button_rgb` for `key_*`/`color_*`/`tint_*`, plus the `params_grid(..., skip)` mechanism (`src/apps/kt-app/src/widgets.rs:123`, `src/apps/kt-app/src/fx_editor.rs:565`). And, most importantly, the ASSEMBLED PREVIEW FRAME is already in RGBA in the UI's hands: `compose_preview_into(Tick(key_tick), &mut rgba)` — `src/apps/kt-app/src/app.rs:3135`. So "take the colour straight from the picture" is a mapping of the mouse position onto a buffer, not a new subsystem.
- **Deterministic noise for dithering already exists**: the counter-based `noise_hash` / `noise_seed(burst_index)` — `src/libs/kt-fx/src/video.rs:459`, byte-identical on CPU and GPU (R.29).
- **Precedent for "an effect with its own point editor" exists, and it has been BURIED**: `FxInstance::CamMotion { keyframes: Vec<CamKeyframe> }` with its own `DragValue`s in `src/apps/kt-app/src/fx_editor.rs:708`; it was removed from the template list (R.34 K2, `fx_editor.rs:515`).

### What is missing

- A "curve" type in the parameter passport. `ParamSpec { name, min, max, default, step, unit, kind }` (`src/libs/kt-model/src/project.rs:2203`) describes a SCALAR, and the whole UI generation, reading and writing from `params_for!` rests on it. A curve is not a scalar and does not fit this declaration.
- A post-composite stage. `FramePlan` consists of `layers` (`src/libs/kt-engine/src/video_plan.rs:76`); there is no place in it for "do something with the finished frame" at all.
- Dithering, and any notion of bit depth at all.

### What gets in the way

1. **The automation widget cannot be reused — and the rule in `src/apps/kt-app/CLAUDE.md` ("curves are drawn by ONE widget `automation::draw`; no effect may have its own keyframe editor") is affected here and needs a decision from the owner.** The argument is not a matter of taste but comes from the code in `src/apps/kt-app/src/automation.rs`: `AutoCtx` carries `range: (Tick, Tick)`, `to_timeline`, `to_curve` and `frame_step`; `graph()` draws the TIMELINE ruler (`crate::timeline::ruler_steps`, `format_ruler_time`), the red line of the main cursor, a gesture on the ruler hands `out.seek` to the main playhead, and a new point snaps to the frame grid. The vertical window is computed FROM THE KEYS (`value_window`, 15 % margin) and is frozen during a drag. For a colour curve the X axis is not time: there is nowhere to put the cursor, nowhere to seek, nothing to snap to; the window on BOTH axes must be strictly 0..255, otherwise "do nothing" stops being the diagonal — so a floating scale here is a direct defect. Keeping one widget means removing from it the ruler, the cursor, seek, snapping and auto-window, after which only an empty rectangle with points remains. The honest answer: the rule is about the KEYFRAME editor (axis = time), and it needs to be worded exactly that way. The cost of mixing is visible in VEGAS: there a colour curve point really did inherit the keyframe type (Linear/Hold), and users ask why the "type of new key" setting does not apply to it — a question without an answer, because "hold" means nothing for a colour curve. What MUST be reused is the common base: placing a point by clicking on empty space, dragging, deleting by double-click, a 9 px grab zone, the selected point in white. It has to be EXTRACTED from `automation.rs`, not copied: item 221 (`slowed` and `pan3d` drifted apart because each had its own editor) is exactly about this.
2. **The curve does not get through the frame plan.** `video_plan.rs`: `fx: Cow<'a, [FxInstance]>` — "the chain WITH ALREADY APPLIED automation (R.30): curves are resolved ONCE when the plan is built". `ParamCurve` collapses into ONE `f64` via `curve_value` / `apply_fx_auto` (`kt-model/src/project.rs:1652`, `:1673`) BEFORE the executor sees anything. A colour curve must reach the executor WHOLE. This is not "one more parameter" but a new kind of data inside `FxInstance`.
3. **GPU: one bind group layout for the whole engine.** `src/libs/kt-gpu/src/engine.rs:458` — a uniform with `min_binding_size: 128` plus EXACTLY three texture slots T0/T1/T2 (`texture_entry(1..3)`), and the `Params` structure is declared in `shaders/common.wgsl`, which is concatenated into ALL ten pipelines. A table of 256 inputs × 4 channels is 1024 bytes, which does not fit into a 128-byte uniform under any packing. There are exactly two options: put the LUT in a texture (pool format `Rgba8Uint`, 256×1 fits without a single change to `pool.rs`) — but that is a fourth slot, or taking T2 away from the passes that use it; or grow the uniform — which is a change to the shared header, meaning recompilation and revalidation of all passes. Either option touches the whole engine, not one shader.
4. **Eight bits is a property of the device, not a "quality setting" (invariant No. 11).** A curve in 8 bits is computed once into a table of 256 inputs and applied by indexing — the cheapest thing we have. But every steep curve merges levels, and on a gradient this shows as bands: exactly the complaint quoted above about skies. The cure is dithering, that is NOISE, and it must be deterministic: `rand` would break byte-identity and GOLDEN_VIDEO. The building material already exists: the same counter-based `noise_hash` (`kt-fx/src/video.rs:459`), byte-identical on CPU and GPU. Not a blocker, but it is a decision that has to be made BEFORE the first kernel, not after the first complaint. Separately: the bands from Sat vs Sat on 12-bit BRAW show that the softness of the selection's falloff is as mandatory a kernel parameter as the curve itself, and "we'll do it later" does not work here.
5. **There is nowhere to hang a grade "on everything".** FX are computed per layer (`video_plan.rs`), and there is no post-composite stage. Through the morpher it does not work either: `MorphPatch.video_fx` is applied in `Project::effective_clips()` (`src/libs/kt-model/src/project.rs`) BEFORE the plan is built, so it lands on EACH clip separately — the same hole described in F-MPH-007. Until it is closed, "the colour of the whole scene" means N identical curves on N clips.
6. **Export thins the chroma AFTER the grade.** `muxer_impl.rs:242` — RGBA→YUV420P: there are half as many colour samples along both axes. Invariant No. 7 is intact (preview and export are byte-identical in RGBA), but a feature-based curve works on the hue of each pixel, and at the boundaries of saturated colours the result in the file will be softer than in the preview. This needs to be known in advance, or a defect "export is coloured differently" will be filed.
7. **The hue domain is closed, and this has to be solved in the model, not in drawing.** In Resolve the curve stops at 0.00 instead of wrapping to 359.99 (quoted above) — so cyclic interpolation has to be set by the curve TYPE in `kt-model`, in the same place where `curve_value` lives today, and closed by a micro-test across the seam. Deferring this to "the second iteration" is not allowed: a non-cyclic hue curve is not a "simplification" but a wrong result on reds.

### Which invariants it touches

- **No. 7 (one composition code).** The formula goes into `kt-fx` and is mirrored in WGSL; under the crate's rule (`src/libs/kt-fx/CLAUDE.md`), the scalar code, the AVX2 kernel and the shader are edited in ONE commit, and the SIMD must be byte-identical to the scalar. There can be no separate version of the curve in the shader.
- **No. 11 (determinism).** New golden references next to t26; GOLDEN_VIDEO and GOLDEN_AUDIO on projects without a curve must not move. Dithering uses only the counter hash.
- **No. 12 (the UI heats the system only while something moves).** The 256-entry table is recomputed when the curve CHANGES, not on each UI frame; previewing the curve in the editor is no reason to touch `cache.epoch`.
- **UI rules**: 1 (a point must be grabbed the first time, the grab zone is wider than the drawing — the complaint about being unable to hit a point after the eyedropper in Lumetri is exactly about this), 8 (the editor is drawn in the language of the program, not dropped in as a ready-made control), 9 (no "R:", "G:" labels — the colour of the line is the label), 13 (a second graph does not get a second strip — tabs inside one rectangle), 17 (the eyedropper and clicking on the graph are paired ways of placing a point, and they must give identical results).
- The rule "one curve widget" — affected directly, and it is discussed above.
- No. 1, 2, 4, 5, 6 are not affected: neither audio, nor the cache, nor decoder ownership, nor the clocks are touched by this.

## Estimate

| What | Answer |
|---|---|
| Size | L — a whole round. These are TWO different kernels (the per-channel table and selection by feature) and TWO different editors, not one effect with a checkbox |
| Where the main work is | `kt-fx` (scalar + AVX2), `kt-gpu` (the shader and the decision where to put the LUT — a texture slot or growing the uniform), `kt-model` (a "curve" type next to `ParamSpec`, which today describes only scalars, plus the cyclic hue domain), `kt-app` (a second curve editor + the eyedropper on the preview frame) |
| Project format | needs envelope version v5 (`KT_META_VERSION` = 4, `src/libs/kt-otio/src/meta.rs:17`); an old build must reject the file via `KtMetaTooNew`, not read it "almost the same way" |
| Tests to write | golden references for the new effects next to t26 (`kt-fx/tests/video_fx.rs`); SIMD parity next to t198; CPU↔GPU conformance next to t181 (only through `assert_frames_close`); no change to t11/t67 on projects without a curve; round-trip and v5 next to t192/t222; cyclic interpolation of hue across the 0°/360° seam — as a separate micro-test; editor layout and the grab of a point after the eyedropper — next to t221 in `layout_tests.rs`; repeatability of two runs with dithering enabled |
| Depends on | — . But F-MPH-007 determines whether a "grade on everything" will appear: without it the curve lives only on a clip |
| Breaks compatibility | forward no, backward yes |

## Verdict

We take it. Colour correction and LUTs entered the roadmap by the owner's direct choice (`docs/user-feedback.md`, R.18 item 6), and curves are the part of it that costs least in our integer 8-bit path: a 256-entry table fits onto the existing video-FX road without a single new concept in the engine.

The order is mandatory: first RGB/Luma — one kernel, one table, one window; feature-based curves as a second step. They are expensive not because of arithmetic but because of three things, each of which is broken in competitors and observable: a point on them is a TRIPLE (the centre plus two selection boundaries), the hue domain is CLOSED, and the eyedropper must take the colour FROM THE PICTURE. What to copy is not the list of five curves — VEGAS has that already since 2022 — but exactly these three properties: without them it becomes one more tool that "exists" but nobody uses.

Two decisions are made BEFORE the code and not during the round: (1) the rule "exactly one curve editor" is reworded as "exactly one KEYFRAME editor" — otherwise the colour curve either gets someone else's time axis or guts the shared widget; (2) whether dithering is on or off — it is the only answer to banding, and it must be counter-based, otherwise invariant No. 11 dies.

## Sources

- https://support.apple.com/guide/final-cut-pro/use-the-color-curves-effect-verf87ca7b62/mac (Luma/Red/Green/Blue, a free-form curve and the eyedropper that picks a colour from the viewer; accessed 2026-09-07)
- https://support.apple.com/guide/final-cut-pro/use-the-huesaturation-curves-effect-ver463347c6b/mac (six curves; the eyedropper places THREE points, the outer ones limit the hue range; accessed 2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/color (Custom Curves + Hue Vs Hue / Hue Vs Sat / Hue Vs Lum / Lum Vs Sat / Sat Vs Sat; picking a value by clicking in the viewer; accessed 2026-09-07)
- https://www.liftgammagain.com/forum/index.php?threads/hue-vs-hue-as-compared-to-using-secondaries.3743/ (curves against secondary correction in colourists' eyes; 2014)
- https://forum.blackmagicdesign.com/viewtopic.php?t=33024 (Hue vs Lum breaks the frame where Hue vs Hue and Hue vs Sat work; 8 bit; 2015)
- https://forum.blackmagicdesign.com/viewtopic.php?t=165970 (bands from Sat vs Sat on BRAW — harshness of the falloff, not bit depth; 2022)
- https://forum.blackmagicdesign.com/viewtopic.php?t=197921 (hue curve stops at 0.00 instead of 359.99; 2024)
- https://community.adobe.com/t5/premiere-pro-discussions/problem-with-lumetri-curves/td-p/10422834 (stray tint from the Lumetri curves, switch to the old RGB Curves; 2019–2022)
- https://community.adobe.com/t5/premiere-pro-discussions/hue-vs-sat/m-p/15548381 (Hue vs Sat in Lumetri, eyedropper, point cannot be grabbed with the mouse; 2025)
- https://creativecow.net/forums/thread/color-banding-only-in-vegas-pro-not-other-softwares/ (banding in VEGAS and none in Premiere on the same source; advice to grade in Premiere and return to VEGAS; 2024)
- https://forum.borisfx.com/t/suggestions-improvements-new-features-boris-vegas-pro-2026/21773 (section "COLOR GRADING CURVES IMPROVEMENTS": 256-level steps, lines at 16 and 235, end points movable along X, export to LUT; 2026)
- https://forum.borisfx.com/t/hold-type-of-video-effect-as-a-default-point-in-vegas/22713 (Linear/Hold point types in the OFX Color Curves; the Color Grading panel does not support keyframing "(yet)"; 2026)
- https://forum.borisfx.com/t/banding-noise/22231 ("bug involving how levels are handled in 8 bit"; 2026)
- https://www.vegascreativesoftware.com/vegas-pro/ (the composition of VEGAS colour tools: white balance, HSL sliders, curves, custom LUTs)

Caveats on the honesty of the check (all access dates 2026-09-07):

- **Adobe's help was unavailable this time**: `helpx.adobe.com` and `www.adobe.com` refused every request (proxy). The presence of a Curves section with Hue vs Sat and an eyedropper in Lumetri is confirmed by SUPPORT THREADS (`community.adobe.com`), not by documentation; the full list of Lumetri curves and the exact behaviour of its eyedropper (how many points it places) must be confirmed against the help in the next wave. The three-point mechanism is documented so far only by Apple.
- **The After Effects row relies on the generally known set of the Curves and Lumetri Color effects**, not checked against Adobe's help this time.
- **The forum `vegascreativesoftware.info` no longer exists**: all its addresses return 301 to the root of `forum.borisfx.com`. The threads have moved (search works), but the DIRECT LINKS to the old threads are dead — including those given as sources for F-AUT-001. The practical consequence for the base: the layer of VEGAS-user complaints before 2022 (that is, before feature-based curves appeared there) cannot be recovered from anywhere else, and the conclusion "VEGAS does not have this" CANNOT be drawn from today's forum.
- None of the five editors was actually launched live: everything above is at the level of trust 1–2 from `docs/features/_editors.md`.
