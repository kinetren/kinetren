---
id: F-CLR-001
title: Lift / Gamma / Gain (and Offset) wheels
status: candidate
priority: A
cost: L
category: color/primary
editors: [resolve-20, premiere-26, vegas-pro-2026, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-fx, kt-gpu, kt-engine, kt-ffmpeg, kt-otio, kt-app]
invariants: [7, 11]
ui_rules: [4, 8, 13, 17, 20]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-CLR-001 — Lift / Gamma / Gain (and Offset) wheels

## Essence

Three or four colour wheels with a point in the middle. Dragging the point changes the BALANCE of red/green/blue; a ring or slider under the wheel changes the overall LEVEL. Each wheel is tied to its own band of brightness: Lift — shadows, Gamma — midtones, Gain — highlights, Offset — the whole frame at once. The editor does no calculations: he sees that the shadows have gone blue and drags the bottom wheel towards yellow. This is PRIMARY correction — it acts on the whole frame, unlike secondary correction, which works on a selected colour or mask.

We have none of this at all. `FxInstance` in `src/libs/kt-model/src/project.rs` has 13 variants (2 audio, 11 video), and among them there is no brightness, contrast, saturation or gamma: the closest to colour are `Greenscreen`, `Edges` and `Glow`, and in all three of them colour is an input parameter, not something that gets adjusted.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| DaVinci Resolve | 20.3.2 | Color page, Primaries palette — switchable sets **Color Wheels**, **Primaries Bars**, **Log** and **ASC CDL** | four wheels Lift/Gamma/Gain/Offset, under each of them four numbers R/G/B/**Y** and a master ring; next to them Contrast/Pivot, Sat, Hue and **Lum Mix**. The **Log** set replaces the controls with Shadow/Midtone/Highlight/Offset and adds MOVABLE boundaries of the bands (Low Range / High Range) — the same gesture, different mathematics. The ASC CDL set is the same controls under the names Slope/Offset/Power (plus Saturation), for exchanging a grade with someone else's pipeline as numbers rather than as a picture. All of this lives in a node: a graph can contain many nodes, and the grade of one is not equal to the sum of the grades of two |
| Adobe Premiere | 26.x | the **Lumetri Color** panel, section **Color Wheels & Match** (plus Basic Correction, Creative, Curves, HSL Secondary, Vignette) | THREE wheels — Shadows / Midtones / Highlights, each with a vertical brightness slider on the left. There is no Offset wheel. In the same panel, the Basic Correction section gives a completely different paradigm of tones — Exposure/Contrast/Highlights/Shadows/Whites/Blacks plus Temperature/Tint — and both sections stack within one effect, top to bottom |
| VEGAS Pro | 2026.0 | the **Color Grading** panel (wheels + curves, since VEGAS Pro 14) and the old plugins **Color Corrector** and **Color Corrector (Secondary)** | two generations of tools live side by side, and the new one can NOT do everything: a Boris FX employee on the forum advises using the old plugin, because the new Color Grading panel does not support keyframing at this point — an animated grade is done with the tool the panel was meant to replace. The old plugin has three wheels Low/Mid/High and eyedroppers for "make this grey". Bit depth is a property of the PROJECT (8-bit / 32-bit float video levels / 32-bit float full range), and the picture changes with it: full range is composited in linear light (gamma 1.000) |
| Final Cut Pro | 12.3 | **Color Wheels** (Master / Shadows / Midtones / Highlights) and the legacy **Color Board** | before 10.4 there were no wheels AT ALL: Apple released FCP X with the Color Board — three tabs (Color / Saturation / Exposure), each with four "pucks" dragged over a rectangle where the horizontal is hue and the vertical is amount. In 10.4 (December 2017) Color Wheels, Color Curves and Hue/Saturation Curves appeared, and the Color Board remained for old projects. Each wheel has TWO sliders on its sides: brightness and saturation — the things that in Resolve are hidden as numbers under the wheel |
| After Effects | 26.x | the **Lumetri Color** effect (the same as in Premiere), plus Levels, Curves, Color Balance, Exposure | the wheels come entirely from Premiere together with the whole panel; AE has no wheel tool of its own — its native pair for the same work is Levels (black, white and gamma points per channel) and Curves |
| Kinotrain | — | **none** | no wheels, no brightness, no contrast, no saturation, no gamma. Colour is adjusted only indirectly: the `opacity` of an event and the weights of transitions |

## The trick behind it

**1. A wheel and the handle under it are not "coarse and precise" but two different operations, and that is exactly why Resolve has FOUR numbers under each wheel, not three.** A circle is two-dimensional: it has two degrees of freedom, while there are three channels. Dragging the point can only be done so that the channels spread in DIFFERENT directions while their sum stays roughly constant. This is pure balance: the colour changes, the overall level does not. The ring (in FCP a slider at the side, in Premiere a vertical bar) moves all three channels in ONE direction: the level changes, the balance does not. Hence the fourth number `Y` under the wheel is the handle of the ring, written out explicitly, and it is not derived from R/G/B.

Then the subtlety begins, which is why Resolve has the **Lum Mix** slider. Brightness is not an independent fourth channel — it is a FUNCTION of the three: move the shadows into blue and the picture gets darker, because blue has a small weight in Y. Lum Mix decides whether to compensate for this or not: at one end a purely colour movement of the wheel leaves the brightness alone (the engine pulls the level back), at the other end the brightness follows along, as it happened. The practical consequence, which appears in no list of features: **the same movement of the wheel gives a different BRIGHTNESS depending on the position of a slider that has nothing to do with the wheel**, and an editor who has not found it will think the wheels "eat the contrast".

**2. The wheels are orthogonal — but this is a property of an UNTOUCHED frame, not of the handles.** The separation works through anchors: Lift is anchored on white (it moves black, white stays), Gain is anchored on black (it moves white, black stays), Gamma moves neither. The anchors are the numbers 0 and 1, not "your black" and "your white". As soon as Lift has moved black away from zero, Gain stops being neutral to the shadows: it multiplies, and multiplying a non-zero value means it must be moved. Hence the most frequent complaint, "I turned it back, and it did not return to where it was": in an editor with 32-bit floating point, a handle returned to its default restores the picture exactly (the order of stages is fixed and does not depend on the order of touches), but two wheels that the user turned one after the other chase each other — you fix the shadows, the highlights drift; you fix the highlights, the shadows drift. And a third layer of the same complaint is the widget itself: the wheel is RELATIVE, each drag adds an offset to the current value rather than setting an absolute value, as a slider does. "Returning it by eye" is therefore impossible in principle: returning is yet another addition, and the numbers accumulate drift (the complaint about the Offset wheel in Resolve — "the more you try to adjust, the lower it goes" — is exactly about this). The only exact return is a reset of the wheel, not a reverse movement; this is why reset is available on each wheel separately. This is also why professionals have a strict order: first the white balance and the black and white points, then contrast, and only then the wheels. White balance is a diagonal matrix; if it is done AFTER a power function, it no longer amounts to one number per channel: grey in the shadows and grey in the highlights require different corrections, and neutrality is chased endlessly.

**Offset is not "the same as Lift, only stronger".** Lift is a shift with white anchored, that is a tilt: it raises black and does NOT touch white. Offset is a transfer of the whole characteristic: it raises black and white alike, and everything between them by the same amount. The difference is visible exactly at white: after Lift the sky stays white, after Offset it goes into blow-out. This is why Offset stands as a separate wheel next to the three: it is the only one that SHIFTS the picture rather than deforming it, and therefore the only one with which you adjust the exposure of the whole take without touching the proportions within the frame.

**3. The "Log" wheels are the same three handles whose band boundaries have been UNPINNED, and here is why this matters for log material.** In Lift/Gamma/Gain the boundaries are fixed: Lift holds to the bottom of the scale, Gain to the top, Gamma to the middle — and this is reasonable when the picture is already displayable (gamma ≈ 2.2–2.4) and its tones are spread over the whole scale. Log material is spread differently: it has no real black and no real white, everything is compressed into a narrow strip around the middle. If you pull Lift on it, it affects not the shadows but half the frame, because "the bottom of the scale" and "the shadows of the scene" have stopped coinciding. The Log set provides Low Range and High Range — points with which the editor himself says where, for THIS material, the shadows end and the highlights begin — and after that Shadow/Midtone/Highlight work by the scene, not by the scale. So the switch Color Wheels ↔ Log in Resolve is not "two skins of one tool": identical numbers give different pictures, and they do not convert back and forth.

## Pros

- **Three handles instead of two is not a convenience but a different dimension.** "Brightness + contrast" per channel gives an affine mapping (`out = a·in + b`): two degrees of freedom, one line. Lift/Gamma/Gain gives a three-section curve and, per channel, nine degrees of freedom. The task "a tungsten lamp yellows the shadows, while daylight from the window hits the highlights in blue" cannot be solved on a line at all, and it is solved with one layer of wheels — no masks, no secondaries.
- The anchors 0 and 1 are chosen so that "setting a black point and a white point" is done in two movements, not iterations: while the picture is untouched, Lift and Gain do not interfere with each other.
- The operation is POINT-WISE: the value of an output pixel depends only on the input pixel. There is no radius, no neighbours, no edge cases; it parallelises perfectly; and — an important consequence for the 8-bit pipeline — in 8 bits it is EXACTLY expressed as a table 256→256 per channel, so it is identical on the scalar code, on AVX2 and in the shader by construction, not by agreement.
- One tool, two professions: the same gesture and the same mathematics serve both technical neutralisation (removing a stray tint) and artistic "look" (shadows in turquoise, highlights in orange).
- All parameters are numeric, so automation comes for free: in our code `params_for!` in `src/libs/kt-model/src/project.rs` supplies `ParamSpec`, and `src/apps/kt-app/src/automation.rs` draws a curve for any parameter without a single line of code per effect.

## Cons

- **Orthogonality ends at the first movement** (see "The trick behind it", item 2): the anchors are absolute 0 and 1, not the current black and white of the frame. The price is iterations between the wheels and the feeling that the tool "does not return".
- **The scale is linear, perception is not.** The same shift is seen at once in the shadows and not at all in the highlights; near the centre of a wheel the movement is more sensitive than towards its edge. Hence the two prosthetic handles that everyone has: a precision multiplier (a slow drag with a modifier) and "reset one wheel", without which it is easier to wreck a wheel than to set it.
- **There are two sets of handles, and they do not translate into each other.** The Log set is not a display mode but different mathematics with movable boundaries: a project graded in one set cannot be shown in numbers of the other. The user sees two similar screens and must know how they differ — the interface does not make this clear.
- **Exchanging a grade loses part of the work.** ASC CDL — Slope/Offset/Power plus saturation, exactly one affine stage and one power stage: everything the editor added on top (curves, secondaries, separate nodes) does not fit into this container and silently disappears on export.
- **A point-wise operation is irreversible in an integer pipeline.** This is not a property of wheels but of bit depth, but they always come together: in 8 bits, `gain 2.0` maps codes 0…127 to 0, 2, 4, …, 254 — 127 of the 255 codes become unreachable, and a smooth gradient of sky turns into steps; `lift −0.05` collapses codes 0…12 into one, and there is no longer any way to restore them with a reverse movement. The first problem is banding, the second is loss of shadows, and both appear the moment the editor touches a wheel.
- **A wheel requires pointer capture and UNLIMITED dragging**, and this clashes with the host panel. Adobe's Lumetri wheels for a year had an open bug report: you pull the mouse, the point travels five millimetres and stops; in the first version of FCP the wheels did not accept keyboard input of numbers. Both breakdowns are not carelessness but the price of a control whose gesture does not fit inside its own rectangle.
- **A rewritten tool loses what the old one could do, and both live side by side.** In VEGAS the new Color Grading panel does not support keyframes, so animated correction is done with the former OFX plugin — the product has two dissimilar colour correctors at once, and one chooses between them not by taste but by whether animation is needed.
- A grade is the only class of effect that is adjusted CONTINUOUSLY while looking at the frame. Everything in an effect that is computed by a hash of parameters (caches, memoisation, reordering of frames) is recalculated for every pixel of mouse movement on a wheel.

## What users say

The argument "Lumetri vs Resolve" — and that very "I am switching because of this":

> When it comes to color adobe is still stuck in 2008. […] Honestly on the
> brink of switching to davinci resolve entirely now that they're improving
> the product fast.
> — Jon Cena, Lift Gamma Gain forum, thread «Lumetri Hell», 04.05.2018,
> https://www.liftgammagain.com/forum/index.php?threads/lumetri-hell.10680/

> Honestly, I hate having to work in Lumetri. I find for even the simplest
> color I rather roundtrip to Resolve.
> — Jordan Mena, same thread, 25.04.2018

The COLOR WHEEL itself as a widget is a separate sore subject, and this is not a colorists' nitpick: the request hangs on Adobe's official bug tracker and is confirmed by different people:

> If you put your mouse cursor over one of them, click and drag it to change
> the color, the mouse moves about 5 milimetres and stops even though you are
> dragging it further on.
> — AndrewTheGreat, Adobe Community, «P: Premiere Pro v25 — Lumetri color
> wheel bug», 15.10.2024,
> https://community.adobe.com/t5/premiere-pro-bugs/p-premiere-pro-v25-lumetri-color-wheel-bug/idi-p/14924922

> Any slider i need to move it's as if it gets stuck and stops moving, then
> take your finger off the mouse and the slider jumps all the way to either
> the left or right.
> — SHOOT FIRST CREATIVE, same thread, 26.11.2024

Direct confirmation of «The trick behind it» № 2 (the handles are not independent) — from a thread about how FCP X 10.4 first got wheels instead of the Color Board:

> I just tested it and the wheels don't work the same as the Board or the
> Curves. The highlights pull up the shadows and vice versa.
> — Dave Jenkins, Creative COW, «FCPX v.10.4 “Color wheels” defect»,
> 03.01.2018, https://creativecow.net/forums/thread/fcpx-v104-color-wheels-defect/

The closest to "turned it back, and it returned to the wrong place" — about the Offset wheel:

> The issue I seem to be having with Offset (Primaries - Color Wheels) is that
> the RGB values slowly decrease in value as you use it. […] The more you try
> to adjust, the lower it goes.
> — Scott Mclean, Lift Gamma Gain, «The Curious Case of the LGG Offset Wheel»,
> 19.09.2022,
> https://www.liftgammagain.com/forum/index.php?threads/the-curious-case-of-the-lgg-offset-wheel.17366/

And the explanation from the same place of why Offset behaves not like Lift at all:

> Offset is an add function. If the result of that function is further
> modified by a non-linear curve — as it is in a color managed situation […]
> those results are modified as well.
> — Michael Most, Lift Gamma Gain, «ACES and Offset», 07.05.2022,
> https://www.liftgammagain.com/forum/index.php?threads/aces-and-offset.17012/

Banding after correction — exactly our case, together with the diagnosis "dithering was skipped when going down to 8 bits":

> I get banding when rendering DNxHD SQ 1080 25p from Premiere Pro CC. Some
> of the footage is apparently 10 bit, and on those clips you can see ugly
> lines in the sky, on flat surfaces etc.
> — Adam Kawecki, Lift Gamma Gain, «PP CC vs 10 bit FS7 footage and export as
> 8 bit», 27.09.2017,
> https://www.liftgammagain.com/forum/index.php?threads/pp-cc-vs-10-bit-fs7-footage-and-export-as-8-bit.9645/

> I suspect that unchecking the maximum bit depth box, which positively
> impacts performance, takes some shortcuts in to converting the 10 to 8 bit
> video and does not include dithering.
> — Cary Knoop, same thread, 27.09.2017

"The grade falls apart on export" — in 2026 still a live topic:

> To put it simply, I have been working on deliveries for a couple of years
> now and I have been finding the problem that when a export is played on VLC
> looks different from the Quicktime player or the Apple Preview.
> — Juan Ropero, Lift Gamma Gain, «Export looks different depending on
> Player», 12.01.2026,
> https://www.liftgammagain.com/forum/index.php?threads/export-looks-different-depending-on-player.19621/

Vegas — the same bit depth, only moved into the project settings, and the user diagnoses himself:

> Seems to be nothing related to the colour space issue, ie for me 8bit levels
> blowing out bright areas. Need to change to 32bit floating video levels to
> get correct balance?
> — andyrpsmith, Boris FX forum (Vegas Pro), «Patch: Vegas Pro 2026 Build
> 105», 14.05.2026,
> https://forum.borisfx.com/t/patch-vegas-pro-2026-build-105/23462

And "switched it on once and no longer use it, because the reset wipes everything":

> Of course, resetting the panel wipes out all the other adjustments, wheels,
> and curves you've already made within that panel, It almost makes the color
> grading panel unusable if your trying out different luts with adjustments.
> — Abdali, Boris FX forum (Vegas Pro), «Bug Report b189: Color Grading Panel
> LUT Replacement Issue», 01.09.2026,
> https://forum.borisfx.com/t/bug-report-b189-color-grading-panel-lut-replacement-issue/25314

## How it fits Kinotrain

### What already exists

- **The effect frame — entirely.** `FxInstance` (`#[serde(tag = "kind")]`,
  `src/libs/kt-model/src/project.rs`), the declaration `params_for!` there too
  (range/step/default/unit/smoothing in one place), the chain
  `apply_video_chain_pooled` in `src/libs/kt-fx/src/video.rs`, the AVX2 twin
  in `src/libs/kt-fx/src/video_simd.rs`, the shader mirror in
  `src/libs/kt-gpu/src/shaders/`, the post-FX cache
  (`BurstKind::PostFxVideo { quality, fx_hash }` in
  `src/libs/kt-cache/src/lib.rs`),
  automation of any numeric parameter (`src/apps/kt-app/src/automation.rs`).
  A new video effect is filling in ready-made places, not a new subsystem.
- **A precedent for a non-numeric control next to the numeric grid.** The key color
  of `Greenscreen`, the color of `Edges` and the tint of `Glow` are already drawn with an eyedropper, not with
  three `DragValue`s: `draw_video_fx_params` in
  `src/apps/kt-app/src/fx_editor.rs` puts their names into `skip` and passes
  the rest to `widgets::params_grid`. The wheel fits into this same seam.
- **A layer for the whole edit already exists.** The grade in `MorphPatch::video_fx`
  (`src/libs/kt-model/src/project.rs`) with the targets `Below`/`Areas` — this is
  "color a piece of the timeline with one movement", the thing for which in other
  editors one places an adjustment clip.
- **A single composition code** (`VideoRenderer::frame_plan`,
  `src/libs/kt-engine/src/video.rs`) — preview and export will see the grade
  identically by construction; there will be no separate "render version".
- **Deterministic noise is already written.** The standard remedy for banding in
  8 bits is dithering, that is, adding noise with an amplitude of a few code units before
  quantization (Cary Knoop's complaint above is exactly about its absence). Ours
  `Noise` is computed by a counter-based hash from `timeline_burst_index`
  (`src/libs/kt-fx/src/video.rs`, `noise_seed`) and matches byte for byte
  on CPU and GPU, so it is usable as dither without breaking invariant 11.

### What is missing

The whole tool. There is no effect that changes the tone or colour of the frame as such; there are no scopes, no histogram, no vectorscope to see the result; there is no notion of "colour space" in the model or in the project file. There is also no 2D control: `widgets::param_row` draws exactly one `DragValue` per parameter, while a wheel is one gesture that writes three parameters at once.

### What gets in the way

**1. Eight bits BETWEEN EVERY PAIR of effects, not only at input and output.** On the CPU a frame is `Frame { rgba: Vec<u8> }` (`src/libs/kt-fx/src/video.rs`), and the chain passes bytes from one effect to the next. On the GPU it is literally the same: `pool::FORMAT = wgpu::TextureFormat::Rgba8Uint` (`src/libs/kt-gpu/src/pool.rs`, whose comment says outright "raw bytes 0..255 without unorm losses"), and each pass takes a NEW such texture from the pool (`src/libs/kt-gpu/src/engine.rs`). The compositing is integer too: `alpha_over` and `dissolve_into` in `src/libs/kt-engine/src/video.rs` compute in `u32` with weights `k/4096`. So the internal precision of one shader saves nothing: the grade result is quantised to u8 BEFORE blur, glow and compositing. "Grade → blur → grade" loses precision at every joint. And once more at the output: the muxer hard-codes `AV_PIX_FMT_YUV420P` (`src/libs/kt-ffmpeg/src/muxer_impl.rs`), that is 8 bits with thinned chroma — even a 16-bit path internally would hit this.

Linear-space working, which all five editors talk about, belongs here too. The byte in the file is not an amount of light but light passed through the camera's or the display's transfer function (roughly the power 1/2.2). While the mathematics runs on these bytes, "adding exposure twice" is not twice, and a 50/50 mix of two frames does not give what a 50/50 mix of the light would. For us this is not a hypothesis but a line of code: `dissolve_into` (`src/libs/kt-engine/src/video.rs`) adds gamma-encoded u8 with integer weights `k/4096`, so the midpoint of a cross-dissolve is darker than physically correct. As long as there is no such discussion in the project at all, the wheels will compute in the same non-linear space — which is fine for a "look" and wrong for anything that pretends to be exposure.

**2. We do not know WHAT our RGB means.** The decoder calls `sws_getContext` + `sws_scale` into `AV_PIX_FMT_RGBA` without `sws_setColorspaceDetails` (`src/libs/kt-ffmpeg/src/source_impl.rs`): the matrix and range of the source (BT.709/BT.601, limited/full) are lost at decoding, and on export RGBA→YUV420P is set the same default way. How real this is, can be seen from our own cross-render test: item 158 classifies colour by going through ALL 16 combinations "matrix 601/709 × range limited/full on encoding and decoding". Wheels on top of an undefined matrix are a tool that corrects an error it does not know about: the editor will use the wheel to pull out a tint that the decoder introduced.

**3. `pow()` breaks the triple parity of the formulas.** The kt-fx law: the scalar code, AVX2 and WGSL compute the SAME THING BYTE FOR BYTE (`src/libs/kt-fx/CLAUDE.md`), and FMA, `f32` instead of `f64`, and `round()` substitutions are forbidden — the same file records that WGSL rounds halves to even while Rust rounds away from zero. Gamma is a power function: AVX2 has no `pow`, and `powf` from libm and `pow` from WGSL are not required to agree on any bit. The constraint is strict and it dictates the shape of the solution: while the path is 8-bit, a per-channel curve is a table 256→256, computed once in `f64` and sent to all three executors; then parity holds by construction, and no `pow` appears in the hot path at all. This is the point where 8 bits, from a shortcoming, become a crutch: the table works precisely because there are 256 input codes.

**4. The post-FX cache lives by the parameter hash.** The key is `BurstKind::PostFxVideo { quality, fx_hash }`, test 66. Any movement of the wheel changes `fx_hash`, that is, throws away the whole stock of processed frames of the clip. For other effects this is tolerable (they are adjusted once and left alone); for a grade it is the main mode of work.

**5. A layer colours clips separately, not the composite.** `MorphPatch::apply_to` appends FX to the chain of EACH target clip (`Project::effective_clips()`), and at a cross-dissolve both halves are computed separately (`PlanLayer::Dissolve`, `src/libs/kt-engine/src/video_plan.rs`). For linear kernels this does not matter, but gamma is a non-linear point-wise function, and `grade(mix(A,B)) ≠ mix(grade(A), grade(B))`: a grade on a morpher over a joint will give NOT the picture that the same grade on an adjustment clip in Resolve gives. Analysed in F-MPH-007.

**6. The wheel is the first real 2D control in the effects editor.** UI rule: a numeric parameter is drawn ONLY by `param_row`/`params_grid` over `ParamSpec` (`src/apps/kt-app/CLAUDE.md`, recorded in item 217 — precisely so that ranges do not drift between windows). The wheel must write through `set_param`, so that clamping and rounding match automation, but draw itself. Plus rule 17 (paired methods are identical): the numbers under the wheel and the wheel itself are exactly the case where two ways of doing the same thing must agree to the bit, and one of them is RELATIVE (a drag adds) while the other is ABSOLUTE (`DragValue` sets) — we have not had such a pair for any parameter yet. From other people's complaints follows a mandatory detail: reset on EACH wheel separately, because "return the wheel by eye" does not work anywhere and never has, and one common reset wipes the whole panel (the complaint about Vegas above). Plus i18n catalogues for five languages (`src/apps/kt-app/i18n/`) and a rebuild of the font subsets.

### Which invariants it touches

- **No. 7 (one composition code).** The point-wise effect by itself does not touch compositing — it lives in the layer's chain, like the other ten. The invariant is affected only if the grade is given a place AFTER compositing ("a layer colours the finished frame") — that is already F-MPH-007, a separate decision.
- **No. 11 (determinism).** A direct emphasis. The CPU path is the byte reference: `GOLDEN_VIDEO` and golden t26 are not regenerated; a new effect with identity defaults does not touch them (the rule "the default of a new parameter = the previous behaviour, byte for byte", item 228), but any attempt to raise the bit depth of the path would regenerate ALL the video references at once — t11, t26, t67, GOLDEN_VIDEO, plus the tolerances of the t181/t182 GPU conformance. This is the real price of "making colour right".
- UI rules: **4** (the useful default — the wheel opens in neutral), **8** (the widget is fitted in, not stuck on — a standard egui circle in the middle of a parameter grid will not do), **13** (do not waste space: four circles is a lot, and they must go into an already occupied place, not into a new panel), **17** (the wheel and the numbers under it are paired methods), **20** (nothing grows on its own: the chain must not append a grade to itself).

## Estimate

| What | Answer |
|---|---|
| Size | **L** — a whole round for the tool in the CURRENT 8-bit path. Bit depth and colour management are not included here: that is a separate piece of work of size XL, and it revises invariant 11 |
| Where the main work is | `kt-model` (the `FxInstance` variant + `params_for!`), `kt-fx` (scalar + AVX2), `kt-gpu` (the shader mirror), `kt-app` (the wheel widget, i18n ×5, font subsets) |
| Project format | a conditional version bump by CONTENT, as in items 189 and 222: `FxInstance` is a typed enum with `#[serde(tag = "kind")]`, and an old build meeting an unknown `kind` will not silently lose the grade but will refuse to read the project as a whole (test 12). Files without a grade must stay byte-stable |
| Tests to write | round-trip of the model and `.otio`; byte parity scalar ↔ AVX2 on sizes that are not multiples (t198 style); CPU↔GPU conformance within tolerance (t181 style); no change of `GOLDEN_VIDEO`/t26 with an effect at defaults; behaviour of the post-FX cache under continuous editing (t66 style); a harness test of the window with the wheel (`layout_tests.rs`); the glyphs of the new labels (t100) and the distinguishability of menu items (t171) |
| Depends on | nothing blocking. Adjacent: F-MPH-007 (what the layer sees at its input) — whether a grade on a morpher will match another company's adjustment clip depends on it |
| Breaks compatibility | forward no; backward yes — an old build will not read a project with a grade (see "Project format") |

## Verdict

**We take it, priority A.** Colour correction stands in the roadmap by the owner's direct choice (R.18 item 6 of `docs/user-feedback.md`: "titles and transitions, colour correction and LUTs…"), and an editor without a single way to adjust the brightness of a frame looks unfinished more than an editor without any of the notes already written in this base. The wheels are the right first note of the category: this is the base on which curves, secondaries and LUTs will lie.

But it must be taken with open eyes and in the right order. The tool itself is cheap — it is one more point-wise `FxInstance` in a ready skeleton, and in 8 bits it is even simplified to a table 256→256, removing the `pow()` parity problem between the scalar code, AVX2 and WGSL. What is expensive is something else: the wheel is the first tool that the current precision of the path actively hinders. Between each pair of effects, both on GPU and on CPU, there is a u8, and the YUV↔RGB matrix is not defined for us at all. So the sensible sequence is: first a separate note and work on bit depth and colour management, then the wheels; otherwise the first grade of a sky will bring banding and a complaint "why is the picture different after export", and we will be repairing the path by complaints instead of repairing it in advance.

## Sources

The forums (here live the "trick" and all the negative feedback; all quotations were checked against the page):

- Lift Gamma Gain, "Lumetri Hell" — https://www.liftgammagain.com/forum/index.php?threads/lumetri-hell.10680/
- Lift Gamma Gain, "The Curious Case of the LGG Offset Wheel" — https://www.liftgammagain.com/forum/index.php?threads/the-curious-case-of-the-lgg-offset-wheel.17366/
- Lift Gamma Gain, "ACES and Offset" — https://www.liftgammagain.com/forum/index.php?threads/aces-and-offset.17012/
- Lift Gamma Gain, "PP CC vs 10 bit FS7 footage and export as 8 bit" — https://www.liftgammagain.com/forum/index.php?threads/pp-cc-vs-10-bit-fs7-footage-and-export-as-8-bit.9645/
- Lift Gamma Gain, "Export looks different depending on Player" — https://www.liftgammagain.com/forum/index.php?threads/export-looks-different-depending-on-player.19621/
- Lift Gamma Gain, "Resolve to Premiere Levels problem" — https://www.liftgammagain.com/forum/index.php?threads/resolve-to-premiere-levels-problem.11826/
- Adobe Community, "P: Premiere Pro v25 — Lumetri color wheel bug" — https://community.adobe.com/t5/premiere-pro-bugs/p-premiere-pro-v25-lumetri-color-wheel-bug/idi-p/14924922
- Creative COW, "FCPX v.10.4 'Color wheels' defect" — https://creativecow.net/forums/thread/fcpx-v104-color-wheels-defect/
- Creative COW, "Color wheels – just a little bit more" — https://creativecow.net/forums/thread/color-wheels-just-a-little-bit-more/
- The Vegas Pro forum (moved from vegascreativesoftware.com to the Boris FX site), "Hold type of video effect as a default point in Vegas" — https://forum.borisfx.com/t/hold-type-of-video-effect-as-a-default-point-in-vegas/22713
- The Vegas Pro forum (Boris FX), "Patch: Vegas Pro 2026 Build 105" — https://forum.borisfx.com/t/patch-vegas-pro-2026-build-105/23462
- The Vegas Pro forum (Boris FX), "Bug Report b189: Color Grading Panel LUT Replacement Issue" — https://forum.borisfx.com/t/bug-report-b189-color-grading-panel-lut-replacement-issue/25314

Our code and our documents (checked by grep, not from memory): `src/libs/kt-model/src/project.rs`, `src/libs/kt-fx/src/video.rs`, `src/libs/kt-fx/CLAUDE.md`, `src/libs/kt-gpu/src/pool.rs`, `src/libs/kt-gpu/src/shaders/common.wgsl`, `src/libs/kt-engine/src/video.rs`, `src/libs/kt-cache/src/lib.rs`, `src/libs/kt-ffmpeg/src/source_impl.rs`, `src/libs/kt-ffmpeg/src/muxer_impl.rs`, `src/apps/kt-app/src/fx_editor.rs`, `src/apps/kt-app/src/widgets.rs`, items 158, 217, 228, `docs/user-feedback.md` (R.18 item 6).

Access date for all links — 2026-09-07.

**Caveats about completeness, so that the next reader does not take them for verified.** Reddit (r/editors, r/premiere, r/davinciresolve), forum.blackmagicdesign.com and discussions.apple.com could not be reached in this pass — the sites did not open; the feedback was gathered from Lift Gamma Gain, Creative COW, the Adobe Community and the Vegas Pro forum on the Boris FX site (the forum moved from vegascreativesoftware.com). The composition and names of the panels in the "Who has it and how" table come from official help and the general layout of the products; they were NOT clicked through live in this pass. Two details are worth re-checking before work begins: the exact set of numeric fields under the Resolve wheel (R/G/B/Y) and Blackmagic's wording about Lum Mix.
