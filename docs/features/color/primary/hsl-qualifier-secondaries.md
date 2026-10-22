---
id: F-CLR-003
title: Secondary correction: HSL qualifier, windows and tracking
status: candidate
priority: B
cost: XL
category: color/primary
editors: [resolve-20, premiere-26, fcp-12, vegas-pro-2026, after-effects-26]
we_have: no
crates: [kt-model, kt-fx, kt-engine, kt-gpu, kt-cache, kt-otio, kt-app]
invariants: [4, 5, 7, 8, 11, 12]
ui_rules: [1, 2, 6, 13, 16, 20]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-CLR-003 — Secondary correction: HSL qualifier, windows and tracking

## Essence

Primary correction turns a knob for the WHOLE frame. Secondary correction first selects a piece of the frame and grades only that: "the sky is blue", "warmer skin", "this one red sign in the corner — mute it". Selection comes in two kinds: by COLOUR (a Hue/Saturation/Luminance range with thresholds and soft edges) and by PLACE (a power window: an ellipse, a rectangle, a gradient, a Bézier shape, or one that follows an object through tracking). They work as a pair: colour answers "what", the window answers "where", and their intersection is what the word "grading" means as opposed to "brightness".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| DaVinci Resolve | 20.3.2 | Qualifier (HSL/RGB/LUM/3D) + Power Windows + Tracker | THE BENCHMARK. The matte is a separate signal of the node ("key"): each node has a Key In input and a Key Out output; the qualifier and the windows add up INTO A KEY; the windows can be combined with Boolean operations; `Key Output Gain` weakens a node's contribution as a whole. Three range groups, and they are BUILT DIFFERENTLY: for Hue — `Hue Center`, `Hue Width`, ONE `Hue Soft` and `Hue Symmetry` (the slope of the falloff); for Sat and Lum — not a centre with a width but a pair of boundaries `Low`/`High` and TWO independent softnesses `Low Soft`/`High Soft`. A separate panel for cleaning the matte — Pre-Filter, Clean Black, Black Clip, Clean White, White Clip, Blur Radius, In/Out Ratio, **Morph Operation (Shrink/Grow/Opening/Closing) + Morph Radius**, Denoise, the Shadow/Midtone/Highlight triad, Post-Filter. The qualifier and the window intersect by default; the `Qualifier matte` switch (Matte/Mask) in the Key palette changes the intersection into a sum. The whole path is always 32-bit float. The tracker (Pan/Tilt/Zoom/Rotate/Perspective 3D) carries the window along with an object |
| Adobe Premiere | 26.x | Lumetri Color → HSL Secondary | It has a Key with "+" and "−" eyedroppers, three range sliders with four handles each (the inner ones are the plateau, the outer ones the softness), a mask preview in three modes — `Color/Gray`, `Color/Black`, `White/Black` — plus an UNDOCUMENTED invert-matte button next to them (the most frequent question on Adobe's forum: "the selection doesn't work, the whole video changes" — the answer is that button). In Refine there are exactly two parameters, Denoise and Blur: there is no morphological shrink/grow. The correction after the selection is not just one wheel: an icon above it switches to classic THREE-PART wheels (shadows/midtones/highlights). It is limited in STRUCTURE, not in the number of handles: the internal matte does not leave the effect; there is no second secondary within one instance (a second Lumetri is needed); there is no window inside Lumetri at all (the mask lives on the EFFECT in Effect Controls); the correction after selection is a fixed short list (wheel, Temperature/Tint/Contrast/Sharpen/Saturation); there are no curves there |
| Final Cut Pro | 12.3 | Color Mask (3D / HSL) + Shape Mask + Auto Mask and Magnetic Mask | Masks are a property of a SPECIFIC correction effect. A colour mask comes in two kinds: a "3D mask", which adds or removes surrounding colours with a single Softness slider, and an "HSL mask", which adjusts hue, saturation and luma independently (the same split as Resolve's 3D and HSL). Apple names the combination directly: to limit the area of the image affected by a colour mask, you can add one or more shape masks. The direction of action is set by the Inside/Outside buttons, plus inverting ("flopping") the masked and unmasked areas. Separately there are neural-network Auto Mask and Magnetic Mask and an object tracker |
| VEGAS Pro | 2026.0 | Secondary Color Corrector (plugin v1) + Bézier Masking + Mocha VEGAS | The plugin is old and has barely changed since: an eyedropper to "select a range", Hue/Sat/Lum with boundaries and smoothness, checkboxes "show mask" and "invert", correction with a colour wheel. The plugin has no window of its own: the shape is made with a SEPARATE tool (a Bézier mask with motion tracking; since VP17 it can do perspective), and planar tracking came with the Mocha plugin, which now has its own category on the Boris FX forum |
| After Effects | 26.x | Keylight, Color Range/Extract, layer masks, Roto Brush 3, Mocha AE | A DIFFERENT PARADIGM: the result of a keyer is the LAYER'S ALPHA, not a selection for correction. To "make only the sky blue", the layer is duplicated, everything except the sky is cut out of the copy, the copy is coloured and placed on top — three objects instead of one knob. But Keylight has something nobody else has: a View dropdown with six modes for displaying intermediate signals (Source, Screen Matte, Status, Intermediate Result, Corrected Source, Final Result) |
| Kinotrain | — | none | There is no colour correction at all; there are no masks in any form (a grep of `src/` for `mask`/`matte`/`bezier`/`roto` gives only the timecode input mask and the OTIO `Track`); there is no tracking. The nearest in meaning is `Greenscreen` in `src/libs/kt-fx/src/video.rs`: a degenerate qualifier that can only throw the selected area into transparency |

## The trick behind it

**1. A tool is useless without a SEPARATE VIEW MODE, and there is not one such mode but a family of them — and the one that works is the one that LIES.** The matte is not adjusted by looking at the picture (on the picture, the difference between "selected at 100 %" and "selected at 97 %" is not visible at all), but by a special view. In Resolve this is Highlight (Shift+H) with a switchable background and a separate black-and-white view of the matte; in Premiere there are mask preview buttons right in the section; in the old VEGAS plugin there is a simple "show mask" checkbox. But the most telling mode is `Status` in Keylight: it does not show the matte as it is, it RECOLOURS it so that "almost black" becomes clearly different from black. This is exactly how Clip Black / Clip White are set: by eye, on an honest black-and-white matte, a pixel with the value 3 out of 255 is indistinguishable from zero, yet it is the source of all the subsequent noise. So the first trick of secondary correction is not the eyedropper or the sliders, but a SPECIAL DISHONEST VIEW, and without it none of the five editors can be configured.

**2. The window is needed not to limit the area but to PERMIT a coarse qualifier.** The obvious explanation for the pair "qualifier AND window" is "select the colour and cut off the rest". The real reason is the reverse: without a window, the threshold has to be pulled to its limit, because the same hue also occurs in five other places in the frame — and a pulled-tight threshold cuts out the midtones, leaving a ragged edge. The window removes this constraint: everything the qualifier would catch by mistake already lies outside the window, so the range can be taken WIDER, the softness can be greater, and the matte comes out cleaner — even though none of the qualifier's own knobs is called "quality". This also explains why one thing is not enough: the qualifier falls apart where colours are not separable (the classic "white shirt on a white background" case — described below), while the window falls apart where the object moves, and it is cured by tracking, not by thresholds. The third participant in this pair is INVERSION: "everything except the skin" is set up with the same selection of the skin, not with a second selection of everything else, which is why in each of the five editors inversion is a tick box next to the selection rather than a separate tool.

**3. The noise of a mask is set by the SOURCE FORMAT, not by the algorithm; and "shrinking the mask" is physically impossible without blurring it.** A colour threshold on 8-bit 4:2:0 H.264 is taken not from the picture the human sees: the chroma lives at a quarter of the resolution, so the hue is constant within a 2×2 block, and DCT quantisation makes it wander from frame to frame by a unit or so. A hard threshold turns this wandering into a flickering block pattern — the mask shimmers ON A STATIC SHOT, and raising the threshold does not help: the boundary shifts, but the tremble is not removed. On ProRes 422 (10-bit, 4:2:2) the same threshold gives a clean edge, because there are four times as many levels and the chroma is at full height — so "works on ProRes, falls apart on H.264" is not a question of the algorithm. This is why cleaning the matte exists as a separate panel; and there the second half of the detail is hidden: a selection can be shrunk in TWO different ways, and they are not interchangeable. The morphological one — `Morph Operation` (Shrink / Grow / Opening / Closing) with `Morph Radius`: honest erosion and dilation, the edge moves without blurring, while Opening/Closing plug holes and remove specks. The filter one — `Blur Radius` with `In/Out Ratio`: blur the edge and shift which side the edge lands on; `In/Out Ratio` also works with ZERO blur, filling small holes. Premiere has neither of these two families: in Refine there is only `Denoise` and `Blur`, so there is no way to shrink a selection at all — and that, not the number of handles, is what makes its secondary correction limited.

## Pros

- A matte as a SEPARATE signal (Resolve) allows reuse: the same selection is fed into the input of another node and colours a second time without a second setup. A matte as alpha (AE) does not allow this by construction — the compositor consumes the alpha.
- The pair "colour + shape" covers different failures: where colours do not separate, shape saves the day; where the object does not stay still, shape tracking does. Neither tool alone covers both cases, and this is a consequence of the design, not a shortcoming.
- A range selection automatically follows the content: the sky stays selected when the camera pans, without a single keyframe. A window pays for the same thing with tracking — that is, with keys on every frame.
- In FCP, masks belong to a SPECIFIC correction effect, not to the clip: two grades on one clip each have their own pair of masks, and "the same blur at different strengths in two places of the frame" is expressed without duplicating the clip.
- The matte view is the only part of the tool that computes nothing and writes nothing into the project: it is a pure display mode, which is why it is cheap and why all five have it.

## Cons

- The matte cleanup panel exists precisely because color selection on compressed material is noisy. That is, half of the tool's knobs are compensation for the source format, and on 10-bit material they are not needed; the user, however, sees an equally long list of knobs in both cases and does not know which of them to touch.
- Choke is inseparable from blurring (see "The trick behind it" 3), so "trimming the edge by 2 pixels" inevitably makes the edge softer. This cannot be hidden, and every editor simply lays out both knobs side by side.
- A matte hidden inside the effect (Premiere) locks the secondary into a single instance: a second secondary = a second Lumetri on top of the first, and the order of their application becomes part of the result, although the user asked for "one more selection".
- A window bound to an effect and a window bound to a layer are different things, and in AE this has cost an entire paradigm: a layer mask cuts the WHOLE layer, so a secondary requires a duplicate of the layer, and a duplicate of the layer is a second pass of the decoder over the same material.
- Tracking writes keys into the project by itself. This is the only part of a secondary that produces data without an explicit gesture from the user, and it is exactly this that breaks predictability: re-tracking = overwriting someone else's keys.
- A neural-network mask does not cancel matte cleanup, it only replaces the way of obtaining the matte: Auto Mask in FCP has exactly the same knobs — Shrink/Expand, Erode, Blur. So "smart selection" removes the eyedropper and the thresholds, but leaves the entire second half of the tool; it gives no saving on the cleanup panel.

## What users say

A thread in which all facets of the feature come up in exactly five replies. The task is to recolor a shirt in an interview, that is, the canonical secondary:

> Just wondering why you're turning to Boris for this and not using a
> selective color-corrector in your host? (e.g. Vegas's "secondary color
> corrector" would be a simple fix)
> — RedRob, Boris FX Forum, 04.11.2020,
> https://forum.borisfx.com/t/worflow-question/9472

The author of the topic tries the reference tool and runs into a limit that cannot be cured by any knob of the qualifier:

> I have been testing with the qualifier in Davinci but it didn't work
> very well. The model has got a white shirt filmed on a white backdrop
> so that complicate things.
> — torbjorn, same thread, 04.11.2020

The reply in the same thread is, in essence, a statement of the limit of applicability of color selection:

> White on white is unfortunately a rotoscoping situation every time.
> — maryp, same thread, 04.11.2020

And this is already 2026 and the reverse side: people take a SECONDARY instead of a specialised keyer, because a range threshold with softness turns out to be more reliable than chroma key:

> I find that works more reliably than the chroma key Fx.
> — RogerS, Boris FX Forum, 27.04.2026,
> https://forum.borisfx.com/t/quick-green-creen-compositing-with-built-in-channel-blend-secondary-color-corrector/22928

The last quote matters to us separately: we have exactly a keyer and no secondary, while users of another editor go in the opposite direction.

## How it fits Kinotrain

### What already exists

More than it seems. All four "matte generators" of which a secondary consists are already written in our code — they are simply connected to other consumers.

- **Color selection with threshold and softness** — `greenscreen` in
  `src/libs/kt-fx/src/video.rs` (line 348). Euclidean distance to the
  key color, `tolerance` — the outer threshold, `tolerance − softness` — the
  inner plateau, with a linear ramp between them, plus `spill`. This is literally a qualifier by RGB (the `RGB` mode in Resolve's terms), and it is the only one, because there is no HSL decomposition anywhere in our code. The parameter spec is `params_for!` in `src/libs/kt-model/src/project.rs`
  (~line 2340), the WGSL mirror is in `src/libs/kt-gpu/src/shaders/`.
- **Selection by LUMA with threshold and softness** — the `MODE_LUMA` core in
  `src/libs/kt-engine/src/transitions.rs` (~line 350): it computes the luma
  of the frame and runs it through `front_mask(l, k, softness)`. This is a ready
  LUM qualifier, except its result goes into `mix4(a, b, w)` — the choice
  between two frames.
- **Parametric WINDOWS with softness** — there too: `MODE_IRIS` (~line 317)
  gives a circle, a diamond and a rectangle via distance from the center, `MODE_WIPE`
  (~line 310) — a linear gradient in an arbitrary direction. This is exactly
  the Circle / Rectangle / Gradient power window from Resolve, with
  softness via `smoothstep`.
- **The common piece of all three** — `front_mask(s, k, soft)` in the same file
  (line 229): "coordinate → weight 0..1 with a soft front". One function,
  the CPU reference, the WGSL mirror in
  `src/libs/kt-gpu/src/shaders/composite.wgsl`; conformance is held by t224
  (transition kinds), t181/t182 (GPU against CPU) and t225. So the window math
  is not only written by us but also mirrored and tested — under a different name.
- **Matte blurring** — also present, by accident: `bilinear_into_scalar`
  (`src/libs/kt-fx/src/video.rs`, ~line 270) averages ALL FOUR
  channels, and `blur` preserves `has_alpha`. Therefore `Blur` after
  `Greenscreen` in the chain already blurs the alpha — this is half of the choke.
- **Animation and a tracking-compatible data scheme** — `ParamCurve` and
  `AutoKey { t: Tick, v: f64 }` (`src/libs/kt-model/src/project.rs`,
  ~lines 1611–1627). The key is stored in the MATERIAL tick, and the curve's axis is the event time
  (R.37); curves are resolved once when the plan is built
  (`LayerSrc.fx` in `src/libs/kt-engine/src/video_plan.rs`). The tracking result is a set of scalar curves by frame, and it lands here without a single new entity.
- **A place for the effect** — the `KEY` category is already set up in the registry
  (`src/apps/kt-app/src/fx_registry.rs`, ~line 35); at present it holds one
  `Greenscreen`.
- **Order of computation in our favour.** FX are computed PER LAYER, the composite
  goes after (`src/libs/kt-engine/src/video_plan.rs`, `PlanLayer::One` /
  `Dissolve`; execution — `apply_video_chain_pooled`,
  `src/libs/kt-fx/src/video.rs`, line 61). For a secondary this is exactly
  what is needed: they grade the CAPTURED PLAN, not a finished composite — like the clip node
  in Resolve and like the event chain in Vegas. The morpher layer, meanwhile, appends its effects to the chain of each target (`c.fx.extend(add)` in
  `MorphPatch::apply_to`), so a qualifier from the morpher will run on each clip separately — for color selection this is better than "on the composite", although it differs from expectations of an adjustment layer.

### What is missing

- **Colour correction itself.** There is a way to select, but nothing to paint with: a grep for `saturation|brightness|contrast|gamma|hue|lut` in `kt-fx`, `kt-gpu` and the `kt-model` specs gives not one effect. A secondary without a primary is a selection with nowhere to apply it.
- **A matte as a separate signal.** For us a frame is `Frame { rgba, has_alpha }` and nothing more; the fourth byte is alpha, and the compositor consumes it.
- **A matte display mode does not exist, and there is nowhere to put one**: `draw_preview` (`src/apps/kt-app/src/app.rs`, line 3095) knows exactly one view — the finished composite from `compose_preview_into`; `VideoGpuExec::execute_into` also returns only the final RGBA.
- **Operations on the matte** (denoise, clean black/white, choke with a direction) do not exist: `Blur` blurs the alpha, but there is nothing to tighten the level after blurring.
- **There is no geometry in the model at all.** Our only "area" — `MorphTargets::Areas` (`src/libs/kt-model/src/project.rs`, line 617) — is TIME × TRACKS, not pixels.
- **There is no eyedropper on the frame.** What the comment in `fx_editor.rs` calls an eyedropper is `ui.color_edit_button_rgb` (line 635), the egui palette. You cannot take the colour of the sky FROM THE PICTURE for `Greenscreen`, `Edges` or `Glow`; for a qualifier this is not decoration but the way to set the starting value.
- **There is no tracking in any form.**

### What gets in the way

- **`ParamSpec` handles only scalars.** `src/libs/kt-model/src/project.rs` (line 2203): `{ name, min, max, default, step, unit, kind }` — one number with a range, and on this rests the entire UI (the rule "a numeric parameter is drawn ONLY by `param_row`/`params_grid`") and the entire automation (`ParamCurve.param` — the name of a scalar field). This gives a HARD fork: **an ellipse / rectangle / gradient is 6–8 scalars, which fit the existing machinery in full** (they are drawn, automated, tracked, and go into the GPU uniform with no new entity at all), **but a Bézier shape does not fit at all** — a list of points of variable length cannot be expressed in `ParamSpec`, is not automated by `ParamCurve`, and requires its own editor on top of the preview. The price boundary runs exactly here, not at "masks / no masks".
- **Two effects silently destroy alpha out of ten.** `edges` writes `p[3] = 255` and returns `has_alpha: false` (`src/libs/kt-fx/src/video.rs`, lines 424 and 453); `event_grid` behaves the same way (lines 716 and 737): the chain `Greenscreen → Edges` loses the key without a single warning. `glow` preserves alpha but writes only channels 0..2 (line 688) — a glow added to a knocked-out pixel is invisible. So "a matte in the alpha" is not an option: one needs either a separate channel that lives through the chain, or a matte enclosed inside a single effect (the qualifier and the correction in one slot — like HSL Secondary in Premiere, with all its drawbacks).
- **The path is 8-bit ENTIRELY, and this is not "we'll fix it someday".** Decoding is `sws_scale` into `AV_PIX_FMT_RGBA` with the `SWS_FAST_BILINEAR` flag (`src/libs/kt-ffmpeg/src/source_impl.rs`, around line 928); all GPU textures are `Rgba8Uint` (`src/libs/kt-gpu/src/pool.rs`, line 13); export goes back to `YUV420P`. So we stand precisely in the corner where a qualifier is noisiest (see "The trick behind it", item 3), and our matte-cleaning panel will be not a decoration but a mandatory part.
- **The preview is computed at a DIFFERENT resolution from the export**, and the colour of a pixel changes because of this. `preview_quality` by default is 640×360 (`src/libs/kt-engine/src/player.rs`, line 264); the user chooses a fraction from 1/16 to 1/1 (`src/apps/kt-app/src/app.rs`, line 3490); and the fraction goes straight into `sws_scale` as the target size. Downscaling AVERAGES the noise: a matte tuned on a 1/4 preview will be dirtier on export — and invariant 7 is not broken by this (preview == export byte for byte only at the same Quality). This is the first tool of ours for which "look in the preview" will stop being sufficient, and this will have to be stated to the user explicitly.
- **The post-FX cache key.** `fx_chain_hash` (`src/libs/kt-engine/src/video.rs`, line 153) hashes the Debug of the chain and the geometry — new fields will get in there by themselves, but the matte display mode must NOT get into the same key, otherwise switching the view will throw away the warmed frames.
- **The GPU texture pool is ping-pong over one texture** (`acquire` in `src/libs/kt-gpu/src/pool.rs`, line 57). A separate matte channel that lives through the chain is a second texture per layer in flight, and a revision of the VRAM budget.
- **Tracking breaks the frame-delivery invariants.** It has to read consecutive frames of the source, and the decoder is owned by exactly one MediaWorker (invariant 5), and "scrolling/zooming does not generate decode requests" (invariant 8) is written precisely about the UI not being allowed to poke the decoder. So tracking is a separate high-priority task in the worker's queue, with progress and cancellation, not "we ran through the frames in the UI thread".
- **Format.** Any of the above raises `KT_META_VERSION` (`src/libs/kt-otio/src/meta.rs`, line 17, currently 4) to 5.

### Which invariants it touches

- **No. 7 "one composition code" — and here it works in our favour.** The matte, the window and their intersection are WEIGHTS, and the invariant enumerates weights by name as something computed once in the plan and only executed by the backend. Since the qualifier and the window live INSIDE the layer (FX are per layer, before compositing), they do not introduce a new compositing stage — unlike a track matte, where "a layer takes the alpha of the neighbouring layer", which would require one.
- **No. 11 determinism.** CPU — byte-identical against GOLDEN_VIDEO; GPU against CPU only via `kt_gpu::assert_frames_close`; the default of a new parameter must be the previous behaviour byte for byte (`src/libs/kt-fx/CLAUDE.md`), otherwise 26 goldens of t26 and t67 move. Threshold operations are the worst case for CPU↔GPU tolerances: a discrepancy in the least significant bit BEFORE the threshold gives a difference of a whole pixel AFTER it. The tolerance will have to be formulated as a fraction of disagreeing pixels, not as an amplitude — and this decision should be made before, not after.
- **No. 4 the cache is the only truth about coverage**: the selection parameters enter the post-FX key; the matte DISPLAY mode does not.
- **No. 5 and No. 8** — tracking walks through frames only via the MediaWorker queue; no UI gesture turns into a decode directly.
- **No. 12** — the matte view and the window handles over the preview may neither allocate in the draw path nor keep the repaint going while idle.
- UI rules of `src/apps/kt-app/CLAUDE.md`: **1** (window handles are dragged with the mouse — the grab zone is wider than the drawing), **2** (the eyedropper and the matte view work DURING PLAYBACK, not only when paused), **6** (a window has a min and a max), **13** (the matte view switch sits in an already occupied place of the preview strip, not on a new row), **16** (impossibility is shown as an error: "this shape does not support tracking", not as a greyed-out hover), **20** "nothing grows on its own" — and here is a direct clash: **a tracker by definition WRITES keys on the user's behalf**. So it has to be an explicit action with an explicit result (a "track" button, progress, one undo step, visible keys in the same `automation::draw` widget), not background magic.

## Estimate

| What | Answer |
|---|---|
| Size | XL, and it splits into three HONESTLY separable parts: (a) an HSL qualifier with matte cleaning and a display mode — **M**, because the kernel already exists (`greenscreen` + `front_mask`) and there are no new entities in the model; (b) a parametric window (ellipse/rectangle/gradient) — **M**, because these are scalars and they fit `ParamSpec`/`ParamCurve`/the GPU uniform as they are; (c) a Bézier shape, roto and tracking — **L…XL** each, because they break the parameter scheme and touch invariants 5 and 8 |
| Where the main work is | kt-fx (qualifier + operations on the matte, scalar and AVX2), kt-gpu (WGSL mirrors, a second channel in ping-pong), kt-model (fields and specs; with a window — geometry), kt-engine (the cache key, the plan), kt-app (the matte view, the eyedropper ON THE FRAME, window handles over the preview), kt-otio (v5) |
| Project format | needs v5 (`KT_META_VERSION` 4 → 5); defaults are not serialised — old files stay byte-stable |
| Tests to write | CPU↔GPU conformance on the pattern of t181, but with a FRACTIONAL tolerance (threshold kernels, see invariant 11); "default == previous bytes" on GOLDEN_VIDEO, t26 and t67; the matte as a function (identical input → identical matte) on both backends; the matte display mode does NOT invalidate the post-FX cache (on the pattern of t66); round-trip of the model and reading of files without the new fields; harness tests in `layout_tests.rs` — the eyedropper on the preview, the view switch, dragging the window handles; with tracking — that the task goes through the MediaWorker queue and is cancelled (on the pattern of scheduler/t27) |
| Depends on | primary correction: without it the selection has nowhere to be applied. The note on curves (`F-CLR-002`) and the note on primary wheels come before this one |
| Breaks compatibility | no |

## Verdict

**We take it, but not all of it and not first.** Secondary correction without primary correction is pointless: we have what to select with and nothing to apply. The order is: primary, then the qualifier, then the window, and only then (if at all) Bézier with tracking.

The main conclusion of the reconnaissance is not "we need masks" but **where the price boundary runs**: it does not run between "masks exist" and "no masks", but between SCALAR geometry and ARBITRARY geometry. An ellipse, a rectangle and a gradient are a handful of numbers, and numbers already do everything for us: they are drawn via `ParamSpec`, animated via `ParamCurve`, go into the GPU uniform, and are computed once in the plan for both preview and export. The kernels for them are written and mirrored — `front_mask`/`MODE_IRIS`/`MODE_WIPE` from the transitions. Bézier and roto are not expressed in this scheme by any field, and they drag in a second parameter editor, which the single-widget rule explicitly forbids.

Separately, and before work begins, two of our limitations must be acknowledged that outside reviews will not mention: the path is 8-bit entirely (so the matte-cleaning panel is mandatory, not optional), and the preview is computed at a different resolution from the export (so "set it up and look" will for the first time in the programme stop being sufficient). And one more thing, the cheapest and most underrated: **the matte display mode matters more than half of the knobs**. It is worth making even before the qualifier itself — it would immediately fix `Greenscreen`, which today is configured blind.

## Sources

- https://forum.borisfx.com/t/worflow-question/9472 — the "re-colour the shirt" thread: secondary correction as the standard answer, the limit of the qualifier on "white on white", the verdict "a rotoscoping situation" (accessed 2026-09-07)
- https://forum.borisfx.com/t/quick-green-creen-compositing-with-built-in-channel-blend-secondary-color-corrector/22928 — 2026: the Secondary Color Corrector is used INSTEAD OF chroma key, reported as more reliable than the chroma key effect (2026-09-07)
- https://forum.borisfx.com/ — the VEGAS forum has moved to Boris FX (`vegascreativesoftware.info` returns 301 here); among its categories is a separate "Mocha VEGAS" — planar tracking in Vegas comes as a plugin, not built in (2026-09-07)
- https://forum.borisfx.com/t/vegas-pro-release-history/24284 — the release history of VEGAS: motion tracking with perspective for the Bézier tool is stated already in VP17 (build 321, 23.09.2019) (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/intro-to-color-masks-ver40b008a7/mac — FCP: a colour mask isolates a particular colour in an image; two kinds of mask (the 3D mask with a single Softness against the HSL mask with separate hue, saturation and luma); Inside/Outside; inverting ("flopping") the masked and unmasked areas; and the direct statement of the intersection: to limit the area affected by a colour mask, add one or more shape masks (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/use-the-auto-mask-ver6c261d07c/mac — FCP: Auto Mask analyses the video and isolates recognised objects such as people, facial features, hair, skin, clothing, skies, foliage and animals; and the cleaning controls are the same ones — Shrink/Expand, Erode, Blur (2026-09-07)
- Code (read, not paraphrased by analogy): `src/libs/kt-fx/src/video.rs`, `src/libs/kt-engine/src/transitions.rs`, `src/libs/kt-engine/src/video.rs`, `src/libs/kt-engine/src/video_plan.rs`, `src/libs/kt-gpu/src/pool.rs`, `src/libs/kt-gpu/src/engine.rs`, `src/libs/kt-model/src/project.rs`, `src/libs/kt-ffmpeg/src/source_impl.rs`, `src/libs/kt-cache/src/lib.rs`, `src/libs/kt-otio/src/meta.rs`, `src/apps/kt-app/src/fx_editor.rs`, `src/apps/kt-app/src/fx_registry.rs`, `src/apps/kt-app/src/app.rs`
- `docs/user-feedback.md` R.18 item 6 (colour correction and LUTs on the public roadmap — the owner's choice) and R.29 ("I would like to move towards After Effects, i.e. effects should have a compositing engine")

None of the five editors was launched live. A candid statement of how far the check went: the FCP wording is taken from Apple's help pages VERBATIM; the forum quotes were checked against the raw posts (`/raw/<topic>/<number>`), not against a paraphrase. The construction of the Vegas SCC and Keylight panels is described from the official help and the generally known mechanics of these tools; there are NO verbatim forum quotes from helpx.adobe.com, forum.blackmagicdesign.com and Reddit here — all three were unavailable at the time of writing, and the sought-after feedback "qualifier falls apart on 8-bit footage" could not be confirmed.

**Correction of 2026-09-07 (after writing).** The labels of the Resolve and Premiere handles, which stood here flagged "to re-check", were RE-CHECKED against primary sources: the DaVinci Resolve 20/21 Reference Manual (ch. 137 Qualifier, 138 Power Windows, 146 Key palette) and Adobe's help on HSL Secondary. Three things were corrected: the asymmetry of Hue (centre + width + one softness + symmetry) against Sat/Lum (Low/High boundaries + two softnesses); the full composition of Matte Finesse, which includes the morphological Shrink/Grow — the earlier claim that "you cannot shrink a selection without blurring it, in principle" was WRONG; and the exact names of the mask preview modes in Premiere and the presence of three-part wheels inside HSL Secondary.

Six items about After Effects remain WITHOUT a primary source (the mechanics of Compositing Options → Effect Mask, Roto Brush 3, the Hue/Saturation Channel Range, the parameters of Color Range / Extract / Linear Color Key, the presence of HSL Secondary in its Lumetri, the project bit depth 8/16/32 bpc and its effect on the matte): `helpx.adobe.com` is unavailable, and `web.archive.org` returned 429 on eight attempts in a row for each of the seven pages. They must be filled in from a machine with access to Adobe's help, or with AE 26.x installed — they cannot be filled in from memory in a technical note. The statements "we have / we don't have" were checked by grep over `src/` and by reading the files, not from the brief.
