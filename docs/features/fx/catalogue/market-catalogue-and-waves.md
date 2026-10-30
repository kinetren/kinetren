---
id: F-FX-007
title: The effect and transition catalogue against CapCut and the core editors
status: researched
priority: A
cost: XL
category: fx/catalogue
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, capcut-2026]
we_have: partial
crates: [kt-model, kt-fx, kt-gpu, kt-engine, kt-app]
invariants: [4, 7, 11, 12]
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-EXT-003, F-CMP-002, F-CMP-003, F-FX-005, F-TXT-006, F-MSK-001, F-SPD-002, F-AUT-004]
trackers: []
services: []
checked: 2026-10-28
---

# F-FX-007 — The effect and transition catalogue against CapCut and the core editors

## Essence

Not one effect but the CATALOGUE: what a person who comes from CapCut or a short-form editor
expects to find in the effect and transition panels, grouped by what they make (a punchy zoom on
the beat, a glitch, light leaks, a film look, dust on the lens, animated text), set against what we
have — 46 effect kinds, 33 transitions, 15 generators — so that the catch-up is a ranked list of
waves, not a wish list. The owner (2026-10-27): "we are slipping badly on effects and transitions,
far behind CapCut and the like. We need to catch up."

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Video FX / Transitions windows (OFX bundles) | ~25 transition families with presets (F-CMP-002); the effects are OFX plugins with presets, the breadth comes from bundled third-party packs. Exact counts for 2026 `[NOT VERIFIED]` |
| Adobe Premiere | 26.x | Effects panel → Video Effects / Video Transitions bins | bins by family (Blur & Sharpen, Distort, Stylize, Transition, Generate…); a transition has direction arrows plus Reverse (F-CMP-002). Counts `[NOT VERIFIED]` (helpx.adobe.com does not open from this environment, `_editors.md`) |
| DaVinci Resolve | 21 | Effects Library: ResolveFX, Fusion Titles/Transitions | the widest built-in set of the five, many ResolveFX only in Studio; Fusion templates are compositions exposed as one effect (the same idea as our composition public parameters, F-TL-012). Counts `[NOT VERIFIED]` |
| Final Cut Pro | 12.3 | Effects / Transitions / Titles browsers | ~100 transition presets (F-CMP-002), every preset is a Motion template with published parameters; on-screen controls in the viewer |
| After Effects | 26.x | Effects & Presets panel | not a catalogue of looks but of building blocks (Fractal Noise, Displacement Map, CC effects, Echo, Time Displacement); the "look" lives in Animation Presets and third-party packs |

**Outside the core.** CapCut is the owner's yardstick and the ORIGIN of the preset-catalogue form
that short-form editing expects; none of the five core editors organises its panels this way.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| CapCut | 2026 (desktop; build number `[NOT VERIFIED]`) | Effects panel, Transitions, Text → Animation, Speed → Curve | desktop effect tabs "Stylize", "Distort", "Blur", "Color Grading", "Transitions", "AI Effects" (web: "Trending", "Cinematic", "Retro", "Glitch"); named looks such as Movie Grain, RGB Split, Zoom Shake, Dreamy Glow, Soft Focus; text animation is three preset slots In / Out / Loop with a duration slider; speed is a curve with named presets (Montage, Hero, Bullet, Jump Cut, Flash In, Flash Out) and an automatic beat sync |

**Our catalogue, from the code**: the plugin assembler's closed list (`BUILTIN_IDS`,
`src/libs/kt-plugins-builtin/src/lib.rs`, by each manifest's media) plus the core-owned kinds
(`FxInstance::all()` without its plugin slots and `TransitionKind::ALL` in
`src/libs/kt-model/src/project.rs`; `GeneratorKind::tag()` in `src/libs/kt-model/src/genmedia.rs`).
The guard t978 compares the five lists with them; a plugin registered through another door is not
listed.

- **Audio effects (18):** `gain`, `slowed_reverb`, `filter`, `eq`, `distortion`, `bitcrusher`,
  `stereo_width`, `tremolo`, `auto_pan`, `vibrato`, `chorus`, `flanger`, `phaser`, `delay`,
  `compressor`, `gate`, `limiter`, `pitch_shift`.
- **Layer marker (1):** `layer_mix`.
- **Video effects (27):** `color_grade`, `duotone`, `posterize`, `vignette`, `corner_pin`, `sharpen`,
  `pixelate`, `chromatic_aberration`, `radial_blur`, `drop_shadow`, `halftone`, `lens_distort`,
  `twirl`, `ripple`, `kaleidoscope`, `cylinder3d`, `geometry`, `cam_motion`, `mirror`, `blur`,
  `greenscreen`, `edges`, `noise`, `vhs`, `glow`, `event_grid`, `pan3d`.
- **Transitions (33):** `crossfade`, `wipe`, `iris`, `clock`, `barn`, `luma`, `push`, `slide`,
  `zoom`, `spin`, `whip`, `blur_dissolve`, `pixelate`, `glitch`, `flash`, `dip`, `blinds`,
  `checker`, `blocks`, `spiral`, `split`, `squeeze`, `stretch`, `swap`, `shake`, `film_burn`,
  `light_leaks`, `zoom_blur`, `flip`, `cube`, `door`, `page_peel`, `page_roll`.
- **Generators (15):** `solid`, `text`, `gradient`, `corners`, `checker`, `noise`, `smpte`,
  `timecode`, `transparent`, `rect`, `ellipse`, `line`, `leader`, `table`, `window`.

**What people use, against what we have.** Verdict vocabulary: `have` · `wave Wn` · `deferred
(<dependency>)` · `rejected (<reason>)`. A market cell either cites a source of the list at the end
that was opened live (`(src N)`) or carries `[NOT VERIFIED]`; the guard t980 checks both.

| Group | What the market offers | What we have | Verdict |
|---|---|---|---|
| Text animations | CapCut: In / Out / Loop preset slots (typewriter, fade, bounce, glitch, wave…) (src 3); FCP/Resolve: title templates with published parameters | 5 title modes as curve presets (`TextAnim`: none, typewriter, fade, ticker…, F-TXT-006); public parameters of compositions (F-TL-012) | wave W3 |
| Lens: fisheye corners, flares, dirt, drops | CapCut: lens flare sold as templates and overlays, a fisheye effect without a housing control (src 8); AE: Lens Flare, Optics Compensation; Resolve: Lens Flare, Lens Reflections, Aperture Diffraction `[NOT VERIFIED]` | `lens_distort` only; no housing/corner mask, no flare, no dirt, no drops (the owner's ktprompts item FX 1) | wave W1 |
| Glitch | CapCut: RGB Split, digital glitch, VHS tabs (src 1); Resolve: Glitch, Analog Damage `[NOT VERIFIED]` | `chromatic_aberration`, `vhs`, `noise`; transition `glitch` | wave W2 |
| Light leaks / flashes as an effect | CapCut: light-leak overlays and flash transitions `[NOT VERIFIED]` | transitions `light_leaks`, `film_burn`, `flash`; nothing as an effect on one clip | wave W2 |
| Retro / film | CapCut: Movie Grain, Retro tab (src 1); Resolve: Film Look Creator, Film Damage `[NOT VERIFIED]` | `noise` (grain), `halftone`, `duotone` (sepia preset), `posterize`, `vhs`; no scratches, dust, gate weave, halation | wave W2 |
| Shake / zoom punch | CapCut: Zoom Shake, camera shake presets (src 1) | transitions `shake`, `zoom`, `spin`; `geometry` with curves and segment shapes (a zigzag segment, R.61) — but no one-click shake or zoom-punch | wave W1 |
| Blur / bokeh | CapCut: Blur tab, Soft Focus, Dreamy Glow (src 1); Premiere: Gaussian, Directional, Camera Blur `[NOT VERIFIED]` | `blur`, `radial_blur` (spin/zoom), `glow`; no lens (bokeh) blur, no directional blur | wave W2 (directional blur needs a Revision, see Verdict) |
| Speed ramps | CapCut: speed curve with named presets; AE/Premiere: time remapping `[NOT VERIFIED]` | `slowed_reverb` rate curve, signed rate and freeze (R.57) — no presets | wave W4 |
| Beat sync | CapCut: auto beat sync, beat markers; Premiere: Remix/beat detection `[NOT VERIFIED]` | nothing: markers are manual, no beat analysis | wave W4 (needs offline analysis, first-wave conclusion 1 of `docs/features/README.md`) |
| Masks | every editor: shape and Bézier masks on effects; CapCut: mask shapes on a clip `[NOT VERIFIED]` | none (F-MSK-001…003 candidates) | deferred (F-MSK-001) |
| Overlays (dust, drops on the lens, grain plates) | CapCut: overlay library with blend modes `[NOT VERIFIED]` | blend modes on tracks/clips in progress (F-CMP-003); no procedural overlay generator | wave W1 (procedural), deferred (F-CMP-003) for media overlays |
| Trails / echo / "buffer + blend levels" | AE: Echo, Time Displacement; ktprompts link "buffer effect + blend levels" `[NOT VERIFIED]` | nothing: a video kernel is a pure function of ONE frame | wave W6 |
| Homothety / recursive zoom (Droste) | AE plugins (third-party) `[NOT VERIFIED]`; ktprompts "homothety (backrooms literally)" | `kaleidoscope`, `twirl` (the Warp primitive family) | wave W6 |
| Bump mapping / relief light | AE: Bevel Alpha, CC Glass `[NOT VERIFIED]`; ktprompts link | `edges` only | wave W6 |
| Parallax / 2.5D | AE: 3D layers and cameras; ktprompts "parallaxing — basic AE" `[NOT VERIFIED]` | `pan3d`, cameras (planar quads) | deferred (3D research, ktprompts CORE: "real 3D") |
| Physics for letters, fisheye on rate change | ktprompts FX 2 and the two TODOs `[NOT VERIFIED]` | — | deferred (F-FX-005, F-SPD-002) |
| Voice effects | CapCut: voice changer presets (robot, chipmunk, megaphone, telephone…) `[NOT VERIFIED]` | the nodes exist (`filter`, `eq`, `distortion`, `pitch_shift`, `chorus`), the presets do not; no ring modulator | wave W7 |
| Transitions: trend set | CapCut 2026 trend pages: speed-ramp cuts, beat-synced cuts, glitch, cinematic fade and zoom, smooth wipes, slow motion with motion blur `[NOT VERIFIED]` | 33 kinds cover zoom, whip, glitch, flash, light leaks, film burn, volume kinds; missing: luma wipe with a user image, shape-mask reveals beyond circle/diamond/box, warp transitions (lens/ripple), motion-blurred slide/push | wave W5 |
| AI effects (portrait, sky replacement, style transfer) | CapCut desktop "AI Effects" tab (src 1) | — | rejected (determinism: invariant 11 requires byte-for-byte audio and a CPU reference for video; a model download per machine makes the frame a function of the model file) |

**The owner's FX items (`ktprompts_v26.10`, sections FX and the link list), each with a verdict.**

| Item | Verdict |
|---|---|
| Fisheye lens corners ("the corners of the lens") | wave W1 |
| Dirt / drops / flares on the lens | wave W1 |
| Letters as physical objects | deferred (F-FX-005) |
| Inertia of letters when the event's rate changes | deferred (F-FX-005, F-SPD-002) |
| The "67 niche plugins" video | deferred (the video was not watched; each look it shows is triaged into W1–W7 when its wave opens) `[NOT VERIFIED]` |
| Jag edit by time remap combined with frame interpolation | wave W4 |
| Parallax ("basic AE") | deferred (real 3D, ktprompts CORE) |
| Text blending and shapes that hold sources | deferred (F-CMP-003, F-MSK-003) |
| Homothety ("backrooms literally") | wave W6 |
| Bump mapping | wave W6 |
| Buffer effect + blend levels | wave W6 |
| Zoom driven by the audio | wave W4 |
| Blur on one word of a ticker | wave W3 |
| Fisheye switched on by speed changes (and only while scrubbing) | deferred (F-SPD-002, F-AUT-008) |

## The trick behind it

**1. CapCut's catalogue is presets over few mechanisms, not many mechanisms.** The desktop panel
looks enormous, but its names are LOOKS ("Zoom Shake", "Dreamy Glow", "Movie Grain") built from a
small set of primitives: a transform curve, a blur, a glow, grain, a colour tone and a blend mode.
The same is visible in text: CapCut's In / Out / Loop slots are preset curves with one duration
slider, and a reviewer of the PC version notes the limit of the form — there is no per-character
authoring inside a preset. For us this flips the cost estimate: most of the gap is not new kernels
but **variants and curve presets over kernels we already have** (built-in variants exist since R.54,
presets that carry curves since F-AUT-004), and the genuinely new kernels are few — lens housing and
dirt, flare, film damage, bokeh, a displacement-by-blocks glitch, and the temporal family.

**2. The one group that is NOT a preset is the temporal one, and it collides with our purity rule.**
Trails, echo, "buffer + blend levels", motion blur on a whip and frame-difference glitches all read
frames OTHER than the current one. Our video kernel is a pure function of one frame (F-EXT-003, kept
for video by `plugin-api-stage1`), because the post-FX memo is keyed on it. The way through is not to
admit state but to read the PAST SOURCE: an echo of N frames is a pure function of the pre-FX frames
at t, t−1 … t−N, which already live in the cache keyed by tick (invariant 4) — exactly like F-FX-005's
"simulate from the start at a fixed step, the state at a frame is a pure function". That is a new
plugin declaration (a temporal window over the INPUT), not a relaxation of the rule.

**3. Lens dirt and drops are not overlays, they are luminance-driven.** Real lens dirt is invisible
on a dark frame and blooms around highlights; drops refract what is behind them. A static overlay
plate with a blend mode (CapCut's approach) looks pasted on exactly when the shot moves. The physical
version is a barrier that takes the frame's own highlights (our `glow` already extracts them with a
threshold) and multiplies them by a procedural dirt texture — deterministic, resolution-invariant and
without a media file to ship.

## Pros

- Most of the catch-up is variants and curve presets: a variant is an instance of an existing kind
  (no new command, no format change, R.54), so a wave of thirty looks can cost one new kernel.
- Every new kernel goes into a plugin family crate (`plugin-api-stage1`), so a wave is built and
  reviewed in parallel without touching kt-model.
- Procedural overlays (dust, dirt, scratches, flare) ship no media and stay deterministic; the
  golden references stay closed over the built-in set.

## Cons

- Speed-ramp presets and beat sync depend on offline audio analysis that does not exist yet; without
  it the beat preset is a curve the person must align by hand.
- The temporal family needs a new plugin declaration and a cache read pattern (N past pre-FX frames per
  output frame) — the first effect whose cost grows with a parameter in frames, not pixels.
- Masks are the largest single gap and are not a plugin wave: they change the model (where a mask
  lives, F-MSK-002).
- Two earlier rejections stand in the way of the obvious names: a directional blur and separate
  sepia/invert kinds were rejected in the video-fx design; the waves must either revise them
  explicitly or express them as modes and variants.

## What users say

> What CapCut does not expose is per-character or per-element authoring inside the preset itself.
> — "Text Animation in CapCut on PC: What It Does Well, and Where It Stops", autoae.online, 15.07.2026,
> https://autoae.online/blog/how-to-do-text-animation-in-capcut-pc

The quote is a reviewer's, not a user's complaint; it is used for the trick (presets over
mechanisms), not as evidence of demand. A live user quote about CapCut's catalogue breadth could not
be obtained: CapCut has no public tracker (`_trackers.md`), the search returned vendor SEO pages and
third-party tutorials, and reddit is closed from this environment (`_editors.md`). The demand side is
the owner's own words (`docs/user-feedback.md`, round of 2026-10-27, item 6) and his ktprompts FX
items.

## How it fits Kinotrain

CAPABILITY: openspec/specs/video-fx/ — the catch-up waves are planned in
`openspec/specs/video-fx/design.md` ("The catch-up waves and their rank"); transitions — `openspec/specs/transitions/`,
voice presets — `openspec/specs/audio-fx/`.

### What already exists

- The registries: `FxInstance::all()` and `TransitionKind::ALL` in `src/libs/kt-model/src/project.rs`,
  `GeneratorKind` in `src/libs/kt-model/src/genmedia.rs`; built-in variants and the strength axis in
  `src/apps/kt-app/src/fx_registry.rs` (R.54); chain presets with curves in
  `src/apps/kt-app/src/user_store.rs` (F-AUT-004).
- The kernel families a wave plugs into: the Warp primitive (`GeomPrim`, `src/libs/kt-fx/src/geom.rs`)
  for lens/twirl/ripple; the threshold and blur of `glow` for highlight extraction; the seeded,
  position-pinned noise of `noise` (`tap_noise`, same file) for grain and dust; segment shapes with a
  zigzag (R.61) for shake.
- The plugin API (change `plugin-api-stage1`) that every new kernel is written against.

### What is missing

The groups marked `wave Wn` in the table; masks (F-MSK-001); beat analysis; a temporal declaration in
the plugin API.

### What hinders

- Invariant 7 (one composition path): every new kernel needs a CPU reference and a WGSL mirror held to
  `FrameTol` — the waves are sized by kernels, not by names.
- Invariant 4 (the cache is the truth): the temporal family must read the PAST pre-FX frames, never its
  own output, or the post-FX memo lies.
- Invariant 11 (determinism): rules out AI effects and any effect whose frame depends on a downloaded
  model or a wall clock.
- Invariant 12: a picker with hundreds of looks must keep "repaint only while something moves" — the
  thumbnail service (video-fx) already throttles.

### Which invariants it touches

4, 7, 11, 12.

## Estimate

| What | Answer |
|---|---|
| Scope | XL in total: seven waves, each one change of 1–2 stages |
| Where the main work is | plugin family crates under `src/plugins/` (kernels + WGSL), kt-app (variants, presets, catalog keys ×5) |
| Project format | does not touch it: new kinds are new plugin ids; variants and presets are existing forms |
| Tests that will have to be written | per wave: golden rows (t328-style), conformance (t329-style), the audio harness for voice presets; this change: the catalogue guards t978–t983 |
| Depends on | F-EXT-003 / `plugin-api-stage1`; F-CMP-003 (blend modes) for media overlays; F-MSK-001 for masks; F-SPD-002 for speed presets as retime |
| Breaks compatibility | no |

## Verdict

**Take — in waves, each a future change built as plugin crates, after `plugin-api-stage1`.**
The ranked list (the guard t979 checks that every id it names exists):

1. **W1 — Lens and camera** (the owner's own item FX 1): fisheye housing corners, lens dirt and drops
   driven by highlights, lens flare, camera shake and zoom-punch variants. Depends on F-FX-002,
   F-FX-001.
2. **W2 — Glitch, film and soft looks**: block-displacement glitch, film damage (scratches, dust, gate
   weave, halation), light leaks as an effect, lens (bokeh) blur, a directional mode of radial blur
   (Revision of the video-fx rejection: as a MODE of `radial_blur`, which the video-fx "Deferred" line
   already allows). Depends on F-FX-003, F-CLR-007.
3. **W5 — Transitions catch-up**: luma wipe with a user image, more iris shapes, warp transitions
   (lens/ripple/twirl), motion-blurred push/slide. Depends on F-CMP-002.
4. **W3 — Text animation library**: In / Out / Loop slots as curve presets on the title, per-word
   targets (the owner's TODO "blur on one word of a ticker"). Depends on F-TXT-006, F-TL-012.
5. **W7 — Voice presets**: telephone, megaphone, radio, robot (a ring modulator — the only new node),
   chipmunk/deep. Depends on F-AUD-010, F-AUT-004.
6. **W4 — Speed-ramp presets and beat sync**: Montage/Hero/Bullet/Flash curves for `slowed_reverb`,
   beat markers from offline analysis, audio-reactive zoom through a link. Depends on F-SPD-002,
   F-AUT-008.
7. **W6 — Temporal and relief**: echo/trails and "buffer + blend levels" over past pre-FX frames,
   Droste/homothety as a Warp mode, bump/relief lighting. Depends on F-FX-005, F-CMP-003.

Not taken: AI effects (invariant 11). Deferred: masks (F-MSK-001), parallax and real 3D (the
ktprompts CORE item), physics for letters (F-FX-005).

## Sources

1. https://www.capcut.com/help/recommended-alternative-effects — desktop / web / mobile effect tabs and
   named looks, dated 2026-02-03 (accessed 2026-10-28)
2. https://www.capcut.com/es-es/help/capcut-transitions and https://www.capcut.com/de-de/help/capcut-transitions
   — the 2026 trend list of transitions (search summary, accessed 2026-10-28; the pages themselves were
   not opened `[NOT VERIFIED]`)
3. https://autoae.online/blog/how-to-do-text-animation-in-capcut-pc — In / Out / Loop slots, keyframes on
   position/scale/rotation/opacity, the per-character limit, 15.07.2026 (accessed 2026-10-28)
4. https://www.capcut.com/tools/speed-ramp, https://shotkit.com/velocity-capcut/,
   https://filmora.wondershare.fr/video-editing-tips/speed-ramp-capcut.html — the speed-curve presets
   and auto beat sync; third-party pages disagree on the exact preset list (search summary, accessed
   2026-10-28, `[NOT VERIFIED]` against the app)
5. https://www.capcut.com/de-de/help/update-capcut-pc-for-latest-version — update steps without a
   version number (search summary, accessed 2026-10-28; the page itself was not opened `[NOT VERIFIED]`)
6. `docs/features/compositing/transitions/transition-vocabulary.md` (F-CMP-002) — the VEGAS, Premiere,
   Resolve and FCP transition rows reused here
7. The owner's claims file `ktprompts_v26.10`, sections FX and the link list (fisheye corners, lens
   dirt/drops/flares, letters as physical objects, "67 niche plugins" video, jag edit, homothety,
   bump mapping, buffer + blend levels, audio-driven zoom) — read 2026-10-28; the videos themselves were
   not watched `[NOT VERIFIED]`
8. `docs/features/fx/stylize/lens-housing-dirt-drops-flare.md` (F-FX-008) — the live re-check of the
   lens group made when wave W1 opened (2026-10-30)

No live check of any program was made before W1's re-check (src 8); the Premiere, After Effects and Resolve counts are marked
`[NOT VERIFIED]` for the reasons recorded in `_editors.md`.
