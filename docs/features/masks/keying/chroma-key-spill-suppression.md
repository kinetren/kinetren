---
id: F-MSK-004
title: Chroma key: matte display, despill and edge handling
status: candidate
priority: B
cost: M
category: masks/keying
editors: [after-effects-26, premiere-26, resolve-21, fcp-12, vegas-pro-2026]
we_have: partial
crates: [kt-fx, kt-gpu, kt-model, kt-app]
invariants: [7, 11]
ui_rules: [4, 9, 13, 20]
round: null
point: null
tests: []
depends_on: [F-MSK-001, F-MSK-003]
checked: 2026-09-07
---

# F-MSK-004 — Chroma key: matte display, despill and edge handling

## Essence

An actor is shot on a green background; the background must disappear and the actor must stay. Choosing the color with an eyedropper is the first five seconds of work. Then the actual work begins: the hair is semi-transparent, there is a green fringe on the dark jacket, there are holes in the smoke, a lamp stand intrudes into the frame at the left edge, and after the key all of this has to be looked at NOT as a picture but as a matte — a black-and-white map of transparency, because the eye does not see gray dirt on a black background, while the composition will show it. A professional key is not "remove the color" but a cycle: look at the matte → push black to black and white to white → tighten or relax the edge → remove the spill from the skin and hair → cut out with a mask what the key cannot handle.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| After Effects | 26.x | Keylight (The Foundry) + Advanced Spill Suppressor + Key Cleaner | the industry standard, licensed from another company. The **View** menu, which besides Screen Matte and Final Result has Inside Mask, Outside Mask, Combined Matte, Intermediate Result and **Status** (the complete list of items in the AE version specifically — **[NOT VERIFIED]**, the Foundry documentation describes the Nuke node). A separate group Screen Matte: Clip Black / Clip White / Clip Rollback / Screen Dilate (with subpixel precision) / Screen Softness / Screen Despot Black and White. Despill is controlled by **Screen Balance** and Despill Bias, not by one slider. Two mask inputs: **Inside Mask** (holdout — "firm up the foreground matte to stop print through") and **Outside Mask** (garbage — "a garbage mask to reveal the background"). Replace Method decides WITH WHAT to fill the place from which the green was subtracted |
| Adobe Premiere | 26.x | Ultra Key | the successor of the separate Adobe product Ultra (Serious Magic): according to Wikipedia, "a plugin labeled "Ultra" is now present in Adobe Premiere Pro. It is a powerful but simplified keying solution". The **Output** menu: Composite / Alpha Channel (in Adobe's terms the matte is precisely the "alpha channel"). Three groups of handles: Matte Generation (Transparency and others), Matte Cleanup (Choke, Soften), Spill Suppression (Desaturate). A garbage mask does NOT come as a separate input — its role is played by the mask of the effect itself (a mechanism common to all Premiere effects: an ellipse/polygon/brush with tracking) |
| DaVinci Resolve | 21 | **two different keys**: Qualifier → 3D Keyer (the Color page) and Delta Keyer (the Fusion page) | on Color the key is part of the SELECTION mechanism for correction: strokes over the frame, refinement in Matte Finesse, viewing the selection via Highlight (**[NOT VERIFIED]** against the official manual; the official BMD page about Color says only that "the qualifier palette lets you select and adjust part of an image based on hue, saturation or luminance"). On Fusion it is a real chroma-difference key: "The pre-matte, matte, fringe and tuning tabs give you a complete set of controls to refine the key until it's perfect!", plus the **Clean Plate** node ("especially useful for clips with reflections, hair and smoke") and separate Garbage Matte and Solid Matte inputs right on the node. The matte is viewed with the switch View Mode / the channels of the viewer |
| Final Cut Pro | 12.3 | Keyer (in the help — Green Screen Keyer) | the only one of the five that sets the key ITSELF when the effect is added (auto-sampling); exactly one handle — Strength ("Adjusts the tolerance (core transparency) of the automatic sampling"), the rest is hidden in Refine Key. Three view buttons: Composite / **Matte** ("displays the matte created by the chroma keyer, making it easier to identify areas not keying well") / Original. Refinement is done not with numbers but with GESTURES over the picture: Sample Color draws rectangles, Edges — a line across a problematic edge. Despill is enabled by default: "By default, the Green Screen Keyer effect applies spill removal, which neutralizes any leftover tinting". There is Light Wrap |
| VEGAS Pro | 2026.0 | Chroma Keyer (standard OFX) + Chroma Blur; BCC Chroma Key Studio / Primatte Studio in the senior editions | the simplest of the five: color + Low threshold + High threshold + Blur amount — exactly our scheme "threshold and softness" (the set of parameters is confirmed by users' descriptions of the settings). Difficult cases in Vegas are not fixed with the standard keyer; the tool is changed: either BCC/Primatte, or not a key at all — Secondary Color Corrector and Channel Blend. Whether the standard Chroma Keyer has a checkbox to show the matte could not be confirmed from the primary source — **[NOT VERIFIED]** |

## The trick behind it

**1. Keylight has a display mode that DELIBERATELY LIES, and it is exactly this that makes the key tunable.** All five can show the matte as it is (Premiere — Output → Alpha Channel, FCP — the Matte button, Fusion — View Mode, the hardware Ultimatte — "combined matte view, internal matte view"), but an honest matte is useless exactly where the key falls apart: the difference between alpha 0 and alpha 6 on a black background is not visible to a person, while on a composite it is "dirty background" that will show through on a light backdrop. Therefore Keylight has, separately from Screen Matte, **Status**: "view an exaggerated view of the key so that minor problems are shown clearly" — the almost-black and almost-white are tinted so that the eye NOTICES them, that is, what is output to the screen is not what will be rendered. This turns the role of the viewer over: it stops being a window onto the result and becomes a measuring instrument with its own scale. It follows that the display mode must be on the KEY ITSELF, not on the preview window: "Screen Matte" is the matte of this very node before the masks are mixed in, and "Combined Matte" is after; the preview window cannot know such a difference.

**2. "Tolerance" is not a number but an EQUATION, and in Keylight its coefficient is brought out to a separate handle, Screen Balance: "blue screens tend to work best with a balance of around 0.95 and green screens with a balance of around 0.5".** That is, the key measures not the distance to a color but a color-difference quantity of the form "the key channel minus the mix of the other two", and the proportion of this mix depends on whether the screen is blue or green. The practical consequence, which is visible only after working for a while: one and the same tolerance figure on a blue and on a green screen means a different physical meaning, and a key without a balance will always be worse on a blue screen than on a green one — not because of the operator's settings, but by its construction. Keylight additionally separates Screen Balance (affects the MATTE) and Despill Bias (affects the COLOR, and is selected, according to the documentation, by the hair and skin tone): one and the same greenness is treated differently in the matte and in despill, and this is deliberate.

**3. Despill is not subtraction but REPLACEMENT, and Keylight forces a choice of what to replace it with.** Screen Replace / Inside Replace offer None, Source, Hard Colour and Soft Colour, and Soft Colour is described as an attempt to "modulate the luminance of the resulting pixel so that it matches the original pixel". The point is that if one simply subtracts the green from a pixel, it does not become neutral — it becomes DARK, because the green channel carries most of the brightness; light hair after a naive despill turns into gray tow. Therefore a mature key, after subtraction, returns the brightness to the pixel — with the color of the background, the original color, or a set color. The second edge of the same thought is what lies under the alpha: Keylight's Final Result gives PREMULTIPLIED RGBA, that is, the key itself decides that the color outside the matte no longer exists, and no subsequent filtering can bring the green back to the edge.
## Pros

- The display mode of the matte lives on the effect, therefore in a multi-pass key (Keylight: Intermediate Result) the matte of EACH pass is visible separately; a preview window would show only the sum and would not say which of the passes went wrong.
- Separate Clip Black and Clip White are two independent handles for two independent defects (dirt in the background and holes in the object), and they are tuned in different places of the frame; one "matte contrast" slider would link them and make one thing fixed while breaking the other.
- Clip Rollback exists precisely because clipping is IRREVERSIBLE in information: Keylight has a separate handle "restored after clipping the Screen Matte too aggressively", that is, the tool admits that the user will first overdo it, and gives a way back without forcing one to turn back the other two sliders.
- Inside/Outside masks are INPUTS of the node, not separate layers on top: the mask takes part in the same calculation as the key, therefore the Combined Matte can be viewed before the composite and one can see that the mask cuts exactly what it should.
- Clean Plate (Resolve) removes the main cause of an uneven matte — an unevenly lit screen — BEFORE the key, not by fitting thresholds after it: one node instead of a dozen tolerance corrections by areas of the frame.
- The automation of FCP (the key is set by itself, one handle Strength) covers the typical case in one click, and refinement is done with GESTURES over the picture (a rectangle, a line across the hair) — the numeric sliders do not even need to be opened.

## Cons

- Eight display modes in Keylight are eight ways to forget to switch back; in Ultra Key the same is more dangerous, because Output is a parameter of the EFFECT, not of the window, so a forgotten Alpha Channel travels into the render (not confirmed by a direct source — **[NOT VERIFIED]**, but it follows from the place where the handle lives).
- The "correct" value of Screen Balance differs for blue and for green — so the presets of a key cannot be carried between screens, and a preset taken on a green screen gives a certainly worse result on a blue one.
- The order of operations on the matte is rigid (clip → rollback → dilate → softness → despot in Keylight; pre-matte → matte → fringe → tuning in Delta Keyer) and in the interface it is expressed only by the ORDER OF GROUPS from top to bottom: having mixed it up, the user gets a different result with the same numbers, and the documentation is forced to warn separately ("use it only after Key, Pre Matte/Clean Plate, Matte and Fringe are behaving").
- Despill that is "enabled by default" (FCP) imperceptibly changes the color of the object even before the user has set anything: an attempt afterwards to match the color correction to the background fights with the desaturation already applied.
- Resolve pays for two keys with two different vocabularies: on the Color page Matte Finesse, in Delta Keyer — Matte and Fringe, and these are NOT synonyms; a user who has learned one starts over on the other page.
- Ultra Key "simplified" is not an oversight but a matter of origin: it has no mask inputs, no clean plate, no multipass — a complex frame forces a move to After Effects, that is, changing the program.

## What users say

> Change the Output menu from Composite (the final result) to Alpha
> Channel. This displays the key in shades of gray.
> — Larry Jordan, "Premiere Pro CC: Create a Green-Screen Key", 26.02.2016,
> https://larryjordan.com/articles/premiere-pro-cc-create-a-green-screen-key/

The very first step of setting up a key in the tutorial is to switch the output to the matte; not "if it did not work out", but before any tweaking of sliders.

> I am not new to using Mocha but I am running into an issue with a scene
> where the actors hands move beyond the edge of the green screen.
> — kihlbahkt_e4833, Boris FX forum, 09.05.2016,
> https://forum.borisfx.com/t/2169

Exactly the case for which garbage/holdout masks exist: the key is fine, but the screen ended — and from then on the question is about shape, not color.

> Well that's some pretty nasty footage - poor subject lighting, heavy
> compression and poor green screen setup.
> — Peter McAuley (Boris FX), 12.05.2020,
> https://forum.borisfx.com/t/8058

The diagnosis of a specialist of the key's manufacturer to the question "how to fix the edges and the halo": three causes out of three — on the set and in the codec, not one in the algorithm.

> I find that works more reliably than the chroma key Fx.
> — RogerS, VEGAS Pro forum (Boris FX), 27.04.2026,
> https://forum.borisfx.com/t/22928/3

A remark about the standard chroma key of Vegas in a thread that suggests keying WITHOUT A KEY — through Channel Blend and Secondary Color Corrector. Telling is what exactly is wrong with a simple key "color + two thresholds": not that it does not work, but that it is not trusted on real footage.

What could NOT be found and why: reddit (r/editors, r/premiere, r/VideoEditing) and video.stackexchange.com do not open from this environment, the `WebSearch` budget in the session is exhausted, and the search engines through which the earlier agents went (DuckDuckGo, Brave, Mojeek, Ecosia, searx mirrors) answered with a captcha, 403 or 429; Bing returned results of another market. Therefore the targeted formulations of the customer — "Ultra Key vs Keylight", "green screen edges look terrible", "why does my key look better in Resolve" — are not confirmed by direct quotes: the four quotes above are taken from the platforms that did open (larryjordan.com and the Boris FX forum, where the VEGAS Pro community also lives).

## How it fits Kinotrain

### What is already there

- `fn greenscreen` — `src/libs/kt-fx/src/video.rs:348`. It computes the EUCLIDEAN distance `d` from the pixel to the key color in u8-RGB (in f64), `hard = tolerance − softness`; at `d ≤ hard` the alpha is zeroed, at `hard < d < tolerance` the previous alpha is MULTIPLIED by `(d−hard)/(tol−hard)`, and nothing else is touched. `has_alpha = true` on output.
- Suppression of spill (R.33 C5, item 228 `docs/porting-notes.md`): the parameter `spill` pulls the DOMINANT CHANNEL OF THE KEY toward the larger of the two neighboring ones, and only where `alpha > 0`. `spill = 0` — the former path byte for byte.
- The parameter specification — `params_for!` in `src/libs/kt-model/src/project.rs:2340`: `key_r/key_g/key_b` (0…255, Rgb8), `tolerance` (1…200, default 70), `softness` (0…100, default 25), `spill` (0…1, default 0). All six are `Smooth`, that is, they are automated by curves on a par with the other FX (`src/libs/kt-model/src/project.rs`, t190/t191).
- The key color in the UI is taken with an eyedropper, not with three numbers: `src/apps/kt-app/src/fx_editor.rs:567` excludes `key_r/g/b` from `params_grid`. The category of the effect is `cat::KEY` (`src/apps/kt-app/src/fx_registry.rs:36`).
- The GPU mirror — `src/libs/kt-gpu/src/shaders/greenscreen.wgsl` (in f32), the conformance test t181 runs with the tolerance `FrameTol::THRESHOLD` (`src/libs/kt-gpu/src/lib.rs:35`): up to 0.5% of pixels may diverge BY ANY AMOUNT — these are legitimate outliers at a hard boundary.
- The key has NO AVX2 kernels, and this is a decision, not a gap: the measurement gave ×1.29 against the usefulness threshold of 1.3 (item 198 `docs/porting-notes.md`), LLVM vectorizes the loop itself. Therefore a change of the formula is scalar + WGSL, without a third copy.
- The reference — `src/libs/kt-fx/tests/video_fx.rs`: `greenscreen` stands in the table `GOLDEN` by name (`0x18dd7db23eb92552`) plus the property `t26_greenscreen_keys_out_green_corner`; one more instance is in the chains of `simd_parity`/`golden_export`.
- Composite: FX are computed PER LAYER, then `alpha_over` with a STRAIGHT alpha over black (`src/libs/kt-engine/src/video.rs:908`, the plan — `src/libs/kt-engine/src/video_plan.rs`).
- A useful property worth knowing: two `Greenscreen` in one chain give the INTERSECTION of opaque areas (the second multiplies the alpha of the first) — that is, a two-color key (two screens in the frame) already works, as in the "Two Color Chroma-Key" technique of Premiere. There is no union or subtraction of mattes at all.

### What is missing

1. **Display of the matte — none at all.** `Greenscreen` writes straight into the alpha, and the preview shows the COMPOSITE: `draw_preview` (`src/apps/kt-app/src/app.rs:3095`) pours the result into a texture through `ColorImage::from_rgba_unmultiplied`, therefore the transparent place is simply the background of the egui panel. One cannot tune `tolerance`/`softness` by such a picture: gray dirt in the background and semi-transparency on the shoulders look the same — "nothing is visible".
2. Operations on the matte: clip black / clip white, shrink-grow (choke), blurring ONLY of the alpha, removal of point garbage. Our `Blur` (`src/libs/kt-fx/src/video.rs:332`) blurs all four channels at once — as a "blur of the matte" it is suitable only at the price of blurring the picture.
3. Despill is poor: channel clamping without balance (there is no analogue of Screen Balance — the distance is Euclidean and symmetric across channels), without bias for skin/hair and without "what to replace with" (Replace Method).
4. There are no garbage/holdout masks, because **spatial masks do not exist in the product at all**: `Area` in `src/libs/kt-model/src/project.rs` is TIME × TRACKS, not a shape in the frame. This is the territory of F-MSK-001, and without it half of the real frames (a lamp stand, the edge of the screen, a hand that went beyond the green) cannot be keyed in principle. The second possible source of such a mask is the alpha of a FOREIGN clip (F-MSK-003): in Keylight Inside/Outside are precisely INPUTS, not the node's own shapes.
5. There is no clean plate (compensation of an unevenly lit screen), no light wrap, and no multipass key with an intermediate result.

### What gets in the way

- **The display mode of the matte is the output of the render, not a state of the window.** Invariant 7: the composition code is ONE for the preview and the export (`VideoRenderer::frame_plan`), the backend is a parameter of execution, not a second path. So there are exactly three honest variants, and all three have a price:
  (a) a field of the effect — then the matte travels into the export, as Output = Alpha Channel travels in Ultra Key (a footgun, but one code);
  (b) a parameter of the frame PLAN, which only the preview call sets — closest to our architecture, but it is the first case in the project where the preview and the export ask one code for different pictures, and the UI rule 20 ("nothing grows or switches off by itself") requires that the mode be switched off explicitly and be visible;
  (c) a separate video FX "show alpha" at the end of the chain — cheap, honest with respect to No. 7, but it does not distinguish the matte of THIS key from the total alpha of the layer.
- **Transparent pixels in our code keep the original green RGB.** The alpha is zeroed, the color is not, and `spill` deliberately does not touch pixels with `alpha == 0`. Everything after the key that interpolates the frame works with the STRAIGHT alpha channel by channel: `bilinear_into_scalar` (`src/libs/kt-fx/src/video.rs:251`, and hence `Blur`, `Pan`, `Glow`, `EventGrid`), `dissolve_into` and `transitions::mix_into` (`src/libs/kt-engine/src/transitions.rs:174`). The result: any blur, zoom or transition after a chroma key drags the green back into the edge. This is why Keylight has a Replace Method and a premultiplied Final Result — these are not decorations but protection against exactly this.
- **The key is computed on a PREVIEW-QUALITY frame.** The decoder gives an already scaled frame: `to_rgba(api, frame, width, height)` with `SWS_FAST_BILINEAR` (`src/libs/kt-ffmpeg/src/source_impl.rs:928`), and `Quality` is part of the worker's request (`src/libs/kt-engine/src/worker.rs:47`) and part of the cache key (invariant 4). A threshold selected on a draft is not the same threshold on the export. All five set up the key at full resolution.
- **Color space.** `sws_getContext` is called without `sws_setColorspaceDetails`, that is, YUV→RGBA goes with the default coefficients, while the key measures the Euclidean distance in the NONLINEAR u8-RGB. Professional keys work by color differences, and Screen Balance is a direct consequence of this. The practical cost of our scheme: on a blue screen and on dark saturated tones the threshold behaves worse than on a green one, and the "sphere" around the key is equally wide along all three axes, while the real spread of the screen is an ellipse stretched along brightness.
- **The rule of the crate kt-fx**: the default of a new parameter = the former behavior BYTE FOR BYTE, the goldens (t26, GOLDEN_VIDEO) are not regenerated; a change of the formula is scalar and WGSL in one commit. For the key this is feasible (all new handles are neutral at zero), but it means an explicit "prev" branch in the core, as was done for `spill`.
- **UI**: a numeric parameter is drawn ONLY by `widgets::param_row` according to `ParamSpec` — but "display mode" is not a number, it needs its own control next to it (a precedent — the combo `PanMode` in `src/apps/kt-app/src/fx_editor.rs:580`) plus strings in the five catalogs `src/apps/kt-app/i18n/*.toml`, font subsets (`scripts/subset-fonts.py`) and tests t100/t125.

### Which invariants it touches

- No. 7 (one composition code path) — directly, and at its most sensitive point: a matte display mode by definition makes the preview NOT equal to the export.
- No. 11 (determinism) — new thresholds over the matte widen the zone where CPU and GPU diverge; the key already has the widest tolerance of all (`FrameTol::THRESHOLD`), and every new hard threshold adds pixels on top of it. Exact comparison is only CPU against the committed reference.
- No. 4 (the cache is the single source of truth about coverage) — indirectly: `Quality` is part of the cache key, so "tuned on the draft, got something different on export" is fixed not by the cache but by a decision about preview quality.
- UI rules: 4 (the default is useful — matte display is off), 9 (nothing superfluous: eight modes, like Keylight, are not needed by us), 13 (look for room for the switch in the already occupied FX slot row), 20 (the display mode must not "stay on by itself").

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: (1) matte display, (2) operations on the matte and despill; garbage/holdout are NOT included in the cost |
| Where the main work is | kt-fx (core: display, clip black/white, choke, alpha blur), kt-gpu (WGSL mirror), kt-app (display control + i18n), kt-model (`params_for!` spec) |
| Project format | not touched: new fields with serde defaults and `skip_serializing_if`, like `spill`; the schema version does not grow |
| Tests that will have to be written | t26 must stay on the previous hashes at defaults; new cases in kt-gpu t181 (THRESHOLD tolerance) and in `simd_parity`; matte properties (monotonicity of clip black/white, choke does not go beyond the frame); a test "an enabled matte display does not get into the export" — if we choose variant (b); a layout test for the control in the FX editor |
| Depends on | F-MSK-001 (shape masks) — without it there are no garbage/holdout, so only part of the task is closed |
| Breaks compatibility | no |

## Verdict

We take it, but in parts and in the right order. **First of all — the matte display**, as a separate small stage: right now we have six knobs, none of which can be tuned against anything, and that is not a "shortcoming of the key" but the reason our key is decorative. Second — operations on the matte (clip black/white, choke, alpha blur) and a meaningful despill with brightness return, because without them the matte display would only show the problem without letting us fix it. Garbage/holdout masks — not here: they depend entirely on F-MSK-001, and until then honestly write "partial". A move to a color-difference formula (analogue of Screen Balance) is a candidate for the next wave: it gives quality, but it requires a second branch in the core for byte-for-byte compatibility of old projects, and without the matte display nobody would see its improvement anyway.

## Sources

- Keylight (The Foundry), node reference — modes View, Screen Matte, Screen Balance, Despill Bias, Inside/Outside Mask, Replace Method:
  https://learn.foundry.com/nuke/content/reference_guide/keyer_nodes/keylight.html
  (accessed 2026-09-07). This is Foundry's documentation of the same plugin that ships with After Effects; the Adobe help (`helpx.adobe.com`) does not open from this environment, so the Premiere/AE row relies on it, on larryjordan.com and on Wikipedia.
- Final Cut Pro User Guide, "Use the Green Screen Keyer effect":
  https://support.apple.com/guide/final-cut-pro/use-the-green-screen-keyer-effect-ver40b003bc/mac
  (2026-09-07).
- Blackmagic Design, DaVinci Resolve — Fusion (Delta Keyer, clean plate):
  https://www.blackmagicdesign.com/products/davinciresolve/fusion
  (2026-09-07). The breakdown of the Delta Keyer tabs and its Garbage/Solid Matte inputs, and the warning about mismatched vocabularies with the Color page — from the secondary source davinciresolveclub.com
  (https://davinciresolveclub.com/davinci-resolve-delta-keyer/, 2026-09-07),
  **[NOT VERIFIED]** against the official manual: the PDF Reference Manual was not fetched from this environment, and the blackmagicdesign.com forum returned 403.
- Blackmagic Design, Ultimatte 12 — list of monitor modes ("combined matte view, internal matte view"):
  https://www.blackmagicdesign.com/products/ultimatte (2026-09-07).
- Larry Jordan, "Premiere Pro CC: Create a Green-Screen Key", 26.02.2016:
  https://larryjordan.com/articles/premiere-pro-cc-create-a-green-screen-key/
  (2026-09-07).
- Wikipedia, "Adobe Ultra" (origin of Ultra Key and the phrase "powerful but simplified keying solution"):
  https://en.wikipedia.org/wiki/Adobe_Ultra (2026-09-07); "Chroma key" (definition of spill) https://en.wikipedia.org/wiki/Chroma_key.
- Boris FX forum (the VEGAS Pro community also lives there), threads 2169, 8058, 22928 — quotes in the section "What users say" (2026-09-07).
- On 4:2:0 and keying: RED, "Chroma Subsampling"
  (https://www.reddigitalcinema.com/red-101/video-chroma-subsampling) — the fact itself that chroma is subsampled by half or more; the link between this and key quality is stated directly only in a secondary source
  (https://crispvideo.app/learn/what-is-chroma-subsampling/, "pulling a clean key … is much harder at 4:2:0"), **[NOT VERIFIED]** against vendor documentation. For us this is about the SOURCE, not the algorithm: at 4:2:0 the edge of the subject is smeared over chroma twice as coarsely as over luma, and no despill will restore that; the trick "blur chroma before keying" is currently inapplicable to us — `sws_scale` returns an already finished RGBA (`src/libs/kt-ffmpeg/src/source_impl.rs:928`), and the engine has no access to the YUV planes.
- Parameters of the built-in Chroma Keyer in VEGAS (Color, Low threshold, High threshold, Blur amount) — from descriptions of the settings by users; the official help (help.magix-hub.com, help.vegascreativesoftware.com) did not open from this environment, **[NOT VERIFIED]**.
