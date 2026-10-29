---
id: F-FX-008
title: Lens and camera — fisheye housing, lens dirt, drops, flare, shake presets
status: researched
priority: A
cost: L
category: fx/stylize
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, capcut-2026]
we_have: partial
crates: [kt-model, kt-fx, kt-gpu, kt-app]
invariants: [4, 7, 11]
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-FX-007, F-FX-001, F-FX-002, F-EXT-003, F-AUT-004]
trackers: []
services: []
checked: 2026-10-30
---

# F-FX-008 — Lens and camera — fisheye housing, lens dirt, drops, flare, shake presets

## Essence

Wave W1 of the catalogue (F-FX-007): the things a real lens adds to a picture besides its
distortion. A fisheye shot gets the dark round housing in its corners; dirt on the front element
lights up around bright spots; water drops refract what is behind them; a bright source throws a
flare; a hand-held or punchy camera shakes. The owner (ktprompts FX item 1, translated): with the
lens effect as a fisheye "you cannot put the lens corners there", and "dirt/drops/flares on the lens"
cannot be put on at all — "and they give their own effects".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Lens Flare (OFX), Lens Distortion (Boris FX pack) | flare and lens distortion come from bundled OFX packs; no housing or dirt of its own `[NOT VERIFIED]` |
| Adobe Premiere | 26.x | Lens Flare; Lens Distortion | Lens Flare has a placed Flare Center, Flare Brightness, a Lens Type list (50–300 mm zoom, 35 mm prime, 105 mm prime) and Blend With Original; the centre is keyframed by hand to follow a light (src 3) |
| DaVinci Resolve | 21 | ResolveFX Light: Lens Flare, Lens Reflections, Aperture Diffraction; Lens Distortion | Lens Reflections takes elements of the shot itself as reflections (colour, defocus type, stretch); the three light effects are Studio-only `[NOT VERIFIED]` (the manual pages answer 403; search summary only) |
| Final Cut Pro | 12.3 | Fisheye; Light → Lens Flare | Fisheye is a distortion with amount only (F-FX-002); flare is a Motion-template effect `[NOT VERIFIED]` |
| After Effects | 26.x | Lens Flare, Optics Compensation; CC Mr. Mercury, CC Glass | Optics Compensation adds or removes barrel distortion; drops on the lens are built by hand from CC Mr. Mercury or a drop map driving CC Glass refraction (src 6) |

**Outside the core.** CapCut is the owner's yardstick; game engines are where lens dirt and drops
are built procedurally, which is the form we can make deterministic.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| CapCut | 2026 | Effects → Video Effects → "fisheye"; lens flare and camera-movement TEMPLATES | the fisheye effect has intensity and colour, no housing control (page of 2025-05-15, src 1); lens flare is a template (brightness, colour, position) and an overlay, not an effect (src 2); zoom/shake come as templates ("Slow zoom", "zoomEffect+slowmo") with "intensity, speed and style" (src 7) |
| Unity Post Processing | 3.0 | Bloom → Dirtiness (Texture, Intensity), Threshold | a full-screen dirt texture diffracts the BLOOM, so dirt is visible only where bright pixels are; the threshold decides which pixels bloom (src 4) |
| MotoGP (Shawn Hargreaves' write-up) | post of 2009-04-13 | raindrops on the lens | each drop sprite stores its shape in alpha and a 2D refraction offset in red/green; the shader samples the scene at the shifted position; drops slide, fade, run to the edge when the camera turns fast (src 5) |

## The trick behind it

**1. Dirt is a coefficient on the highlights, not a layer.** Unity's Dirtiness multiplies its texture
into the bloom, never into the frame, and Sonic Ether's "natural bloom" makes the same point: dirt
scatters existing light and adds none, so a flat dark frame shows no dirt at all (src 4). CapCut's
answer is an overlay template blended on top — which shows dirt on a black frame and stays put when
the shot pans. Our `glow` already extracts highlights with a threshold (`src/libs/kt-fx/src/video.rs`,
`glow`), so `lens_dirt` is glow's highlight buffer × a seeded dirt pattern, added back.

**2. A drop is a displacement, so it is an INVERSE map and fits the geometry family.** The MotoGP
implementation keeps only an offset per drop pixel and samples the scene at the offset (src 5); the
After Effects recipes use the drop as a displacement map for the image behind it (src 6). An offset
of the SOURCE coordinate is exactly what our Warp primitive computes (output → input), so drops are a
new Warp mode with seeded drop cells — no new pass, fused with the lens distortion before it, and
resolution-invariant because the drop field is in normalised stage coordinates. A drop on the lens
itself acts as a strong wide-angle lens and flips the image inside it (src 6) — the refraction sign
is a parameter, not a constant.

**3. Nobody detects the light for a flare.** Premiere's Lens Flare is placed at a centre the editor
keyframes onto the light (src 3); Resolve's Lens Flare also places it `[NOT VERIFIED]`. A detector
would make the flare jump between frames; a placed, animatable position is what the market ships and
it stays a pure function of the parameters.

## Pros

- Three of the four kernels reuse an existing mechanism: the housing is `vignette`'s radial mask in
  its own stage's coordinates, dirt is `glow`'s threshold, drops are one more Warp mode next to
  `lens_distort` (F-FX-002).
- Procedural dirt and drops ship no media file and stay deterministic (a counter-based hash, as
  `noise` already uses on CPU and GPU byte for byte).
- Shake and zoom punch need no kernel: curves with zigzag segments (R.61) on `geometry`, carried by
  chain presets (F-AUT-004).

## Cons

- The plugin API of `plugin-api-stage1` does not yet have the three contracts these kinds need: a
  POSITIONED tap (the housing measures its stage coordinate, like `vignette`), geometry primitives in
  the API (drops), and a barrier pass on the GPU (dirt, flare) — recorded by that change's task 4.1.
- A drop mode is a core edit of `geom.rs` and `geom_fused.wgsl` in one commit; the fused map stays
  closed to plugins.
- A flare follows a light only by hand-made keys; there is no tracker (F-MSK masks/tracking).

## What users say

> The drop that you are making is used as a displacement map for the animation behind it.
> — Creative COW forum reply on water drops on the lens (After Effects), https://creativecow.net/?p=5655

The quote is a practitioner's method, used for trick 2, and was read from a search summary (the
thread itself was not opened, `[NOT VERIFIED]` verbatim). A user complaint about CapCut's lens looks
could not be found (no public tracker, `_trackers.md`); the demand is the owner's FX item 1.

## How it fits Kinotrain

CAPABILITY: openspec/specs/video-fx/ — planned in `openspec/changes/fx-lens-and-camera/`.

### What already exists

- `lens_distort` as the Warp primitive mode 0 (`GeomPrim::Warp`, `src/libs/kt-fx/src/geom.rs`) and its
  WGSL mirror `src/libs/kt-gpu/src/shaders/geom_fused.wgsl`.
- `vignette`'s radial mask measured in its stage's coordinates and `glow`'s blurred-luma threshold
  (`src/libs/kt-fx/src/video.rs`); `tap_noise` with the counter-based hash (`src/libs/kt-fx/src/geom.rs`).
- The factory chain presets `src/apps/kt-app/assets/fx-presets-factory.json` — "Punch in" already
  exists (a `geometry` zoom curve plus `sharpen`).
- The plugin family crates `src/plugins/fx-video-color` and the assembler
  `src/libs/kt-plugins-builtin/src/lib.rs`.

### What is missing

A housing mask, a dirt pattern on highlights, a drop field, a flare, and the shake presets.

### What hinders

The three plugin API contracts above (positioned tap, geometry primitives, barrier pass); until they
land the new kinds would have to be enum variants, which `plugin-api-stage1` forbids for new kinds.

### Which invariants it touches

4 (the post-FX memo keys on the parameters — every kernel is a pure function of its frame and
parameters), 7 (CPU reference and WGSL mirror per kernel), 11 (seeded hashes, no wall clock).

## Estimate

| What | Answer |
|---|---|
| Scope | L — 3 stages: housing + presets, dirt + flare, drops |
| Where the main work is | `src/plugins/fx-video-color`, `src/plugins/fx-video-stylize`, `src/plugins/fx-video-distort`; kt-fx `geom.rs` + kt-gpu `geom_fused.wgsl` for the drops mode; kt-app catalogs ×5 |
| Project format | does not touch it: new plugin ids, presets in the existing preset file |
| Tests that will have to be written | golden rows and CPU↔GPU conformance per kernel, the dark-frame rule of dirt, drop determinism, the presets parse and apply |
| Depends on | F-EXT-003 / `plugin-api-stage1` (the three contracts), F-FX-002, F-FX-001, F-AUT-004 |
| Breaks compatibility | no |

## Verdict

**Take, as wave W1 of F-FX-007** (change `fx-lens-and-camera`): housing, dirt and flare as plugins,
drops as a Warp mode, shake and zoom punch as presets. The code waits for the three plugin API
contracts; dirt is driven by the frame's own highlights, never an overlay plate.

## Sources

1. https://www.capcut.com/ja-jp/resource/fisheye-filters — CapCut fisheye: Effects → Video Effects,
   intensity and colour, page of 2025-05-15 (accessed 2026-10-30)
2. https://www.capcut.com/pt-br/explore/lens-flare/7512745546026371089 — lens flare as templates
   (brightness, colour, position) and an overlay (accessed 2026-10-30)
3. https://www.miracamp.com/learn/premiere-pro/lens-flare-effects — Premiere Lens Flare controls and
   the hand-keyed centre (search summary, accessed 2026-10-30; Adobe's own page does not open here,
   `_editors.md`)
4. https://docs.unity3d.com/Packages/com.unity.postprocessing@3.0/manual/Bloom.html — Dirtiness on the
   bloom, the threshold (accessed 2026-10-30); https://discussions.unity.com/t/se-natural-bloom-dirty-lens/536285
   — dirt scatters existing light (search summary)
5. https://www.shawnhargreaves.com/blog/motogp-raindrops-keep-falling-on-my-lens.html — drops as
   refraction offsets, 2009-04-13 (accessed 2026-10-30)
6. https://creativecow.net/?p=5655 and https://www.videocopilot.net/blog/2008/01/new-tutorials-water-drops
   — drops as a displacement map, a drop flips the image (search summary, accessed 2026-10-30)
7. https://www.capcut.com/explore/camera-movement-effect — camera movement templates (accessed
   2026-10-30)
8. https://www.premiumbeat.com/blog/analysis-two-davinci-resolve-fx/ and the Resolve manual mirror
   (mans.io, 403) — Lens Reflections, Lens Flare, Aperture Diffraction, Studio-only (search summary
   `[NOT VERIFIED]`)
