---
id: F-CLR-004
title: 1D and 3D LUTs (.cube) — technical versus creative
status: candidate
priority: B
cost: M
category: color/management
editors: [vegas-pro-2026, premiere-26, resolve-20, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-fx, kt-gpu, kt-engine, kt-otio, kt-app]
invariants: [4, 7, 11]
ui_rules: [8, 11, 13, 16]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-CLR-004 — 1D and 3D LUTs (.cube) — technical versus creative

## Essence

The editor takes a text file `.cube`, puts it on a clip — and the picture changes its colour entirely: a flat grey log becomes a normal Rec.709 or acquires a "cinematic" tint. No wheels, curves or scopes are needed for this: all the work is done in advance, and the file holds a ready table "colour at input → colour at output". A strength slider blends the result with the source.

That is exactly why a LUT is the cheapest colour feature of all: it has no editor of its own. And that is exactly why it is the most dangerous: the file knows nothing about where it was placed.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Color Grading → LUTs (Camera LUT / Look LUT); a separate OFX "LUT Filter" | TWO slots in the grading panel, each with its own file, its own **Strength** and its own **Tetrahedral Interpolation** checkbox. The help itself dictates the order: "first normalize with a Camera LUT, then blend in a Look LUT". Plus the independent OFX plugin LUT Filter, which attaches at four levels — Media, Event, Track, Main output (the levels — F-MPH-005), and there the switch is called "Linear (good) / Tetrahedral (best)". The Camera LUT presets come in pairs — "Log to Linear" and "Log to REC.709". User `.cube` files do NOT appear in the list: they are put inside the installed program (`…\OFX Video Plug-Ins\Vfx1.ofx.bundle\Contents\Resources\AutoLooks\cameraLuts`), otherwise every launch starts with "Browse" |
| Adobe Premiere | 26.x | Lumetri Color: Basic Correction → **Input LUT**; Creative → **Look** + **Intensity** | TWO slots inside ONE effect, on different floors of the panel: the technical one on top, the creative one below. The strength slider is only on the creative one — the standard answer to "weaken my LUT" is "put it on an adjustment layer and lower the layer's Opacity" (the same compositional decoupling as F-MPH-006). Mixing up the slots is widespread: log normalisation gets loaded into Look, a creative preset into Input LUT. In parallel, in 2026 Adobe put a new **Color Mode** with Styles into beta, where a per-clip LUT is NOT yet made at all |
| DaVinci Resolve | 20.3.2 | LUT on a node; LUT on the source clip; the project's Input/Output LUT; Display/Viewer/Scopes LUT | There are not two slots but six, and their order is fixed by the scheme in the manual: Camera Raw → **INPUT LUT** → Color-page processing → **OUTPUT LUT** → Deliver, while the **DISPLAY LUT** branches off only to the monitor and is "never applied to rendered media". Within one step "1D LUTs at each step are always applied before 3D LUTs". The interpolation method is a setting of the WHOLE PROJECT (Trilinear / Tetrahedral), not of the effect. A LUT on the source clip is not carried over by ColorTrace, so the manual advises placing the LUT on a node. The LUT library is a system-wide catalogue (`/opt/resolve/LUT` on Linux) plus a list of additional paths in System Preferences |
| Final Cut Pro | 12.3 | Camera LUT (Info inspector, media level) and the Custom LUT effect | The technical slot is taken out of the effects altogether: a camera LUT lives in the media metadata at the library level and, in Apple's wording, is "applied at the media level (across the library) and are applied before any clip effects". The creative one is an ordinary effect from the browser, and Apple explicitly requires: "Make sure the Custom LUT effect is the last effect in the effects list". That is, the order is set not by the code but by an instruction to the user |
| After Effects | 26.x | the **Apply Color LUT** effect; plus the same Lumetri Color | There are no slots as such: a LUT is an ordinary effect in the layer's stack, and the file is chosen by a dialog when the effect is dragged. The order "technical → creative" is built by the editor themselves through the order of effects or through the floors of adjustment layers (F-MPH-002); the program gives no hint that this LUT is an input one and that one is a look |

## The trick behind it

**1. The interpolation mode of a cube is a switch VISIBLE to the user, and its default was chosen for compatibility with other programs.** A 3D LUT is a lattice of nodes (17³ = 4913, 33³, 65³), and everything between the nodes is inferred by interpolation. Resolve writes about its default Trilinear literally: "backward compatible with grades that use LUTs from previous versions of DaVinci Resolve and matches the look of LUTs being applied in other applications", and about Tetrahedral — "higher image quality LUT and DCTL processing, with reduced color-banding". So the famous "the same `.cube` looks different in Premiere and in Resolve" is neither a legend nor a placebo, but a documented consequence of a setting that in Resolve is set on the PROJECT, in Vegas on EACH instance of the effect ("Linear (good) / Tetrahedral (best)"), and in Premiere is not set at all. And the difference is not visible on every frame: on a flat fill the nodes coincide and both methods give the same result — they diverge on GRADIENTS, in bands. Vegas states this directly: tetrahedral "yields smoother gradients … especially with coarse or high-contrast LUTs".

**2. A LUT has not one slot, and the second stands on another floor of the pipeline.** A technical LUT (log → Rec.709) must go BEFORE creative correction, and the creative one AFTER; therefore in Vegas the grading panel has two separate rows (Camera LUT and Look LUT) with their own files and sliders, in Premiere there is Input LUT in Basic Correction against Look in Creative, and in FCP the technical slot is taken out of the effects stack altogether onto the media level ("applied before any clip effects"), while the creative one is required to be last in the list. Resolve, instead of two slots, has six, and the manual fixes their mutual order by a scheme, including the rule "1D LUTs at each step are always applied before 3D LUTs". A wrong choice of slot gives no error and no warning — only "somehow not as in the example".

**3. The strength slider is NOT on every slot, and this shows in the workarounds.** Vegas gives Strength to both rows and documents it as an honest wet/dry: "The LUT filter produces an image that is a blend of the original image and the color-adjusted image… A setting of 0 would be equal to no LUT being applied", and the help advises keeping the creative LUT "around 0.2 to 0.4". Premiere gives Intensity only to the creative Look — and as a result the typical instruction from LUT vendors for Premiere sounds not "lower the strength", but "create an adjustment layer, put the LUT on it and lower the layer's Opacity". This is exactly the same compositional decoupling instead of a per-effect one, discussed in F-MPH-006: if an effect has no wet/dry of its own, one grows it with a layer.

## Pros

- The feature consists of a parser of a text file and one interpolation formula: no curve editor, no wheels, no scopes, no histogram. It is the only colour capability that has NO UI of its own, apart from a "choose file" button and one number.
- The `.cube` format (Adobe/Iridas) is common to the whole industry: a header `LUT_3D_SIZE`/`LUT_1D_SIZE`, optional `DOMAIN_MIN`/`DOMAIN_MAX`, `TITLE`, then line by line triples of numbers. The file can be read in a notepad — the forum formulation: "they're text files that you can inspect with notepad… the more points as defined in the header, the more accurate". By supporting it the program immediately gets the libraries of camera manufacturers and the whole commercial preset market, without creating its own format.
- A technical LUT makes log material watchable BEFORE any correction: one can edit right away, without having a single grading tool. Vegas advises the newcomer exactly this — "start with Log to REC.709 for a fast, good looking baseline".
- The mixing strength is an ordinary number 0..1, so it falls into the ready mechanisms (a single grid of parameters and automation curves, F-AUT-002) for free, without a separate widget.

## Cons

- A cube has finite resolution, and everything between the nodes is inference. The Resolve manual: "one might have a 17x17x17 LUT that specifies 4913 individual color transforms… color values falling between the … color transforms specified by the LUT need to be interpolated". Hence banding on gradients and a dispute about the interpolation method, which the user must decide for themselves.
- A cube is defined on 0..1, and everything outside is clipped: "when a 3D LUT is fed values that are outside of the range that LUT is designed to handle, the out-of-range data will be clipped… feeding a video signal with super-white in it to a 3D LUT that's designed for full-range data (0–1) will clip the super-white part of the signal". The industry answer is a shaper LUT, that is, a PAIR of a 1D and a 3D, where the 1D brings the signal into range and the 3D returns it. One 3D cube does not solve the task.
- The file does not know what it was fed. A log LUT applied to an already normalised picture and a Rec.709 LUT applied to a log both "work" and both lie; LUT vendors write about this in plain text: "You cannot use Rec709 LUTs here as the result will look very different from the expected outcome". None of the five editors checks conformity or warns.
- The LUT library lives OUTSIDE the project — in the program's directory (Vegas) or in the OS (Resolve). So it does not move with the project and gets lost on reinstalling and updating: in Vegas, user cubes are put inside the folder of a specific major version.
- In 8 bits, a LUT plus subsequent correction gives noticeable banding — the standard Vegas forum answer is "you also need to be in 32-bit mode (at least at the render stage) for maximum quality". The cheapness of the feature and "as in the example" diverge exactly here.

## What users say

> When I want to use them as a Camera LUT, I must click the "Browse for
> Camera LUT" and select the .cube file every time I use Vegas. Can I make
> it persist in the list somehow?
> — ryan4, VEGAS Pro, 16.05.2026, https://forum.borisfx.com/t/how-can-i-make-my-custom-luts-persist-in-vegas/23550

The answer in the same thread — put the cubes inside the installed program, in `…\Vegas Pro 2026\OFX Video Plug-Ins\Vfx1.ofx.bundle\` `Contents\Resources\AutoLooks\cameraLuts`. So "my LUTs disappeared" is not a bug but a design: the library is tied to the program version, not to the project or the user.

> I tried and the Luts generally throw the gamma way off and it's hard to
> compensate for it. More harm than good really.
> — steve6, VEGAS Pro, 24.04.2026, https://forum.borisfx.com/t/guidance-for-hlg-to-rec709-in-vp/22762

The classic "why does my LUT look faded / not right": the user feeds HLG into a LUT designed for a different input and drops the tool for good — "tried it once and turned it off forever".

> You only need to use the built in "LUT Filter" and set the interpolation
> to "Tetrahedral (best)".
> — Steve_Rhoden, VEGAS Pro, 02.07.2026, https://forum.borisfx.com/t/best-lut-ofx-and-best-color-accuracy-and-most-efficient/24444

The standard advice on "colour accuracy" is not to change the LUT but to change the INTERPOLATION. In the same thread: "using an ofx camera log-lut with aces would be a huge mistake" — about putting a technical LUT on top of a picture that has already been converted.

> Now if the source in EXR has a high brightness, after applying ACES to
> R.709 we get a clamp. I would like the exposure and gamma operations to
> be applied first, and only then the 3d lut to be applied.
> — AMP, Boris FX, 21.03.2025, https://forum.borisfx.com/t/request-image-preprocessor-luts-pocess-order/19865

A request exactly about the order of stages: the clipping arises not from a bad LUT, but from the LUT standing in the wrong place of the pipeline.

> How and where do you add and change LUTs on a per clip basis
> — h.sevush, Adobe Premiere (Beta), 18.08.2026, https://community.adobe.com/announcements-727/color-mode-in-adobe-premiere-1626244

The reply of R Neil Haugen (19.08.2026): "That's coming, the thing isn't nearly done yet! Still beta, remember." Adobe, in the announcement of Color Mode (Kevin-Monahan, 02.06.2026), contrasts its Styles precisely with LUTs — "unlike LUTs, every parameter is fully adjustable and non-destructive" — but per-clip LUTs in the new mode do not exist yet.

## How it fits Kinotrain

### What we already have

- **The 8-bit pipeline runs end to end.** CPU: `Frame { w, h, rgba: Vec<u8>,
  has_alpha }` in `src/libs/kt-fx/src/video.rs`. GPU: `pub(crate) const
  FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Uint`
  (`src/libs/kt-gpu/src/pool.rs:13`).
- **The per-layer video-FX chain is one for preview and export** —
  `apply_video_chain_pooled` (`src/libs/kt-fx/src/video.rs`) through the plan
  `src/libs/kt-engine/src/video_plan.rs`, invariant No. 7. The order of effects
  in the chain is already edited by the user ("the first is the one nearest to
  the decoder"), so "before/after correction" is decided for us by swapping slots,
  not by a second slot in the UI as in Premiere.
- **The floor "above the lower ones" already exists** — the morpher (`MorpherEvent`,
  `Project::effective_clips()` in `src/libs/kt-model/src/project.rs`):
  the technical LUT goes on the clip, the creative one on the morpher layer, exactly
  like Camera LUT versus Look LUT (F-MPH-001, F-MPH-007).
- **A single parameter declaration `params_for!`**
  (`src/libs/kt-model/src/project.rs:2308`) gives the specs, reading, writing with
  clamping, the common grid `widgets::params_grid` and the curves
  `automation::draw` — the "LUT strength" gets automation for free (the key domain is the footage, F-AUT-002).
- **Relative paths to files in the project are already solved** —
  `src/libs/kt-otio/src/url.rs` (`media_target_url`, `resolve_media_path`,
  decision R.24 item 7); writing goes through `convert.rs:119`.
- **A model of behaviour when a file is missing already exists** —
  `src/libs/kt-media/src/placeholder.rs` (`PlaceholderSource`: a frame with the
  file name, the project opens, the render goes on); the composition is in
  `KtApp.placeholder_media`, the warning in the render dialog —
  `src/apps/kt-app/src/render_dialog.rs:106` (tests 209/210).
- **A precedent for a non-numeric field in the effects editor** —
  `src/apps/kt-app/src/fx_editor.rs`: the colour picker of Glow, the checkbox of Noise,
  the camera combo of Pan3d are drawn BEFORE the common grid of numbers.

### What is missing

- A parser for `.cube`: `LUT_3D_SIZE` / `LUT_1D_SIZE`, `DOMAIN_MIN` /
  `DOMAIN_MAX`, `TITLE`, then `size³` lines of three numbers each — with a check of
  the number of lines and a clear error on a broken file.
- A string parameter of an effect. `fx_cast!`
  (`src/libs/kt-model/src/project.rs:2216`) handles exactly `f64/f32/u32/u8`;
  a file path will become the FIRST non-numeric parameter of an effect in the system, and
  `ParamSpec` has no place for it.
- The notion of "an external file that an effect references". The only external file of
  the project today is the media from the bin, and even the morpher references it through
  `MorphPatch.media: Option<MediaId>`, not through a path.
- A place on the GPU for the cube itself: the passes bind exactly three textures
  `texture_2d<u32>` (T0/T1/T2, `src/libs/kt-gpu/src/shaders/common.wgsl`)
  with one 64-byte uniform.

### What gets in the way

1. **Cube interpolation is exactly the case where the CPU and GPU must diverge, and the crate has forbidden this.** `src/libs/kt-gpu/CLAUDE.md`:
   "There are no samplers at all: the sub-texel filtering grid wanders across drivers — bilinear by hand (4 texelLoad, a mirror of `bilinear_into`)".
   Hardware trilinear filtering of a 3D texture is exactly what this rule forbids; moreover `Rgba8Uint` is integer and cannot be sampled in WGSL at all. So trilinear (8 nodes) or tetrahedral (4 nodes) will have to be written by hand and kept as a mirror of the scalar and AVX2 kernels of kt-fx in one commit. This will be checked by t181
   (`src/libs/kt-gpu/tests/parity.rs`) via `assert_frames_close`, and the tolerance `FrameTol::BILINEAR` is `max_lsb: 2`, `mean_lsb: 0.5`,
   `outlier_frac: 0.001`: tuning the thresholds to hardware filtering is forbidden (invariant No. 11, "a failure at the edge is an investigation, not a tweak").
2. **The cube has nowhere to go without touching the common layout.** Right now "one uniform for all passes (64 bytes, generic slots) — one bind group layout for the whole engine". A separate `texture_3d` changes the layout of ALL passes; the alternative is to lay the cube out as a tile in the free T2 and address it by hand. A choice between two unpleasant options, and it is architectural, not cosmetic.
3. **The cache key does not see the file's contents.** `fx_chain_hash`
   (`src/libs/kt-engine/src/video.rs:153`) computes
   `xxh3_64(format!("{fx:?}|{auto:?}|…"))`: the path will get into the key, but an edit of the `.cube` itself on disk will not. The post-FX cache (`PostFxVideo`) and the GPU `ReadbackCache` will silently return frames with the old grade, which directly violates invariant No. 4 ("the cache is the single truth").
4. **A missing LUT is not a missing media.** The whole machinery of relative paths and placeholders is tied to the bin: only `kt-otio/src/convert.rs:119` relativises paths, and only `render_dialog.rs:106` warns before a render, both for `media.path`. For a LUT file this would have to be either duplicated, or the LUT recognised as "media of a special kind" — and then it would go into the bin, where by meaning it does not belong.
5. **8 bits.** Our path is u8 throughout, while forum practice requires 32 bits for LUTs at least at the render stage. This does not cancel the feature (in Vegas the same user says that they like the 8-bit LUT more than the 32-bit ACES), but "as in Resolve" cannot be promised on gradients, and the UI must say so honestly.

### Which invariants it touches

- **No. 7** — one composition code: the LUT is computed in the `frame_plan` chain, not "on the GPU in its own way"; an export started on the GPU cannot move to the CPU.
- **No. 11** — determinism: the CPU path is byte for byte against `GOLDEN_VIDEO`, the GPU only through `FrameTol`/`assert_frames_close`; ad hoc epsilons are forbidden.
- **No. 4** — the cache is the single truth about warmth: the key must change when the LUT changes (see point 3).
- The rule of the kt-fx crate: scalar + AVX2 + WGSL in one commit, SIMD byte for byte with the scalar. About "default = previous behaviour" here it is simply: there was no effect before, so the golden references do not move.
- UI checklist: **item 8** (the button for choosing a file is drawn in the same rectangle as the value it replaces, not as a "pop-up" dialog), **item 11** (new strings → font subsets and t100), **item 13** (the LUT's file name goes into an already occupied place, not a new strip), **item 16** (a missing or broken `.cube` is a red error, not a faded hint).

## Estimate

| What | Answer |
|---|---|
| Size | M — 2–3 stages of the round |
| Where the main work is | kt-fx (`.cube` parser + scalar + AVX2), kt-gpu (WGSL mirror and the place for the cube), kt-model (a non-numeric effect parameter), kt-app (file choice, error for a missing LUT), kt-otio (the path in the project file) |
| Project format | a new version of the `metadata.kt` envelope is needed: `FxInstance` gets a string field, and files of past versions must be read as they are (precedent — t212 "reading files up to v3") |
| Tests to write | `.cube` parsing (correct, broken, `DOMAIN_MIN/MAX`, 1D and 3D, sizes 17/33/65); a golden CPU reference for 3D LUT in `video_fx.rs`; SIMD parity `simd_parity`; CPU↔GPU conformance in `parity.rs` (t181) — by tolerance, not by epsilon; the round trip of the model and `kt-otio/tests/format.rs` for the relative path; behaviour with a missing file (after the pattern of t209/t210); t100 for the new strings in all five catalogues |
| Depends on | nothing (it requires neither colour correction nor scopes); the strength slider is the same mechanism as in F-MPH-006, the curve — F-AUT-002 |
| Breaks compatibility | no: old projects are read, the references are not regenerated |

## Verdict

**Take it — as the first step into colour.** The LUT is the only one in its category that requires neither a parameter editor nor scopes: a file plus interpolation, and "colour correction and LUT" stand on the roadmap chosen by the owner himself (`docs/user-feedback.md`, R.18 item 6; the landing-page line "Colour grading: curves, white balance, LUTs and scopes").

Cheap as it is, this is true only up to the point of interpolation: for us the GPU and the CPU must compute ONE thing (invariant No. 7), while kt-gpu has deliberately refused hardware filtering — so trilinear/tetrahedral is written by hand in three copies (scalar, AVX2, WGSL) and checked by t181. This is the price of honest parity, and it is known in advance, not discovered in the middle of a round.

What NOT to bring along next: a separate "Input LUT slot" next to a "Look slot" — for us the order of application is already expressed by the order of effects in the chain and by the floors of the morpher, and a second slot would be a parallel way of doing the same thing (item 17 of the UI checklist). The error of slot choice, on which newcomers in Premiere stumble, we have no reason to create.

## Sources

- VEGAS Pro 2026 Online Help, "LUTs" (Color Grading window): Camera LUT /
  Look LUT, Strength, Tetrahedral Interpolation, `.cube` —
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/8-design/colorgrading_luts.htm
  (accessed 2026-09-07; the page is marked "Last modified on
  April 15, 2026")
- VEGAS Pro 2026 Online Help, "LUT Filter" (OFX): levels of application,
  Interpolation Linear/Tetrahedral, Strength as a mix with the original —
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/microcontent/Microcontent/_FxPlugins/lut-filter.htm
  (accessed 2026-09-07)
- VEGAS Pro 2026 Online Help, "Color Grading" —
  https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/8-design/colorgrading.htm
  (accessed 2026-09-07)
- DaVinci Resolve 20 Reference Manual (Blackmagic Design, July 2025):
  ch. 6 "Project Settings" — the Input/Output/Display LUT scheme, the rule "1D
  before 3D", 3D Lookup Table Interpolation (Trilinear/Tetrahedral),
  LUT catalogues, "33x33x33 cubes with 32-bit floating point"; ch. 150
  "Using LUTs" — shaper LUT and clipping outside 0–1, LUT on the source
  clip versus LUT on a node; ch. 9 "Process Node LUTs in" —
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_20_Reference_Manual.pdf
  (accessed 2026-09-07)
- Final Cut Pro User Guide, "Apply LUTs" —
  https://support.apple.com/guide/final-cut-pro/apply-luts-ver24f966423/mac
  (accessed 2026-09-07)
- Final Cut Pro User Guide, "Use custom LUT effects to color correct
  ProRes RAW video" ("Make sure the Custom LUT effect is the last effect
  in the effects list") —
  https://support.apple.com/guide/final-cut-pro/custom-lut-effects-color-correct-prores-raw-ver572cacef4/mac
  (accessed 2026-09-07)
- Adobe Community, "Color Mode in Adobe Premiere" (Kevin-Monahan,
  02.06.2026; the thread on per-clip LUTs — h.sevush 18.08.2026,
  R Neil Haugen 19.08.2026) —
  https://community.adobe.com/announcements-727/color-mode-in-adobe-premiere-1626244
  (accessed 2026-09-07)
- Boris FX Forum (VEGAS): threads 23550 (persistence of user LUTs), 22762 (HLG → Rec.709, "Luts throw the gamma way off"), 24444
  (interpolation, ACES versus LUT, 32 bits), 19865 (order of stages and
  clipping), 25314 (bug in replacing a LUT in the grading panel, build 189) —
  https://forum.borisfx.com/ (accessed 2026-09-07)
- Lutify.me Docs, "How do I apply 3D LUTs in Adobe Premiere Pro" (Basic
  Correction → Input LUT for log normalisation, Creative → Look for the creative one) and "How can I change the strength/opacity of the 3D LUTs in
  Adobe Premiere Pro" (the workaround via the Opacity of an adjustment layer), "How do I
  apply 3D LUTs in Adobe After Effects" (the Apply Color LUT effect) —
  https://lutify.me/docs/ (accessed 2026-09-07)
- Lutify.me, "How to correctly color grade RED IPP2 footage in Premiere
  Pro" ("You cannot use Rec709 LUTs here…") —
  https://lutify.me/how-to-correctly-color-grade-red-ipp2-footage-in-premiere-pro/
  (accessed 2026-09-07)

**Note on completeness.** On the date of verification `helpx.adobe.com` was unavailable
either directly or through two independent external proxies (a break at the
HTTP/2 level), so the Premiere and After Effects rows rely on the announcement in the official
Adobe community and on the LUT vendor's documentation, not on the Adobe help. At the next revision of the
note, re-check the Premiere/AE rows against `helpx.adobe.com` — above all the presence and behaviour of Intensity on
Creative → Look and the absence of a strength slider on Basic Correction → Input LUT.
