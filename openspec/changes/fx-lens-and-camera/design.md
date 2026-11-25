# Design

## Context

See proposal.md — Why. What exists and shapes the plan (observed on 36d751a7, 2026-11-25):

- **The lens we have**: `lens_distort` is a warp plugin of `fx-video-distort`: `GeoKernel::steps` gives
  `GeoStep::Warp { k: [f64; 7], norm }`, `GeoKernel::warp` is the f64 inverse map the planner measures the zoom with,
  and `warp_<id>(pt, a, b)` in the plugin's own WGSL is the same map in both loops of the fused pass (`a.yzw = k[0..3]`,
  `b.xyzw = k[3..7]`). A new warp is a file in the plugin crate; the former "Warp mode 5, one core edit" is gone.
- **The mechanisms the new kinds reuse**: `tap_vignette` and `tap_noise` are positioned taps: the coordinate is the
  fourth argument, the stage's size is in `b.zw`; `noise_hash` is the counter hash; `glow`
  (`fx-video-blur/src/lib.rs`, `glow_add.wgsl`) thresholds the BLURRED luma with a soft knee through two
  `Node::Resample`.
- **The plugin API today**: `Decl.positioned`, `VideoKernel::cook_at`, `BarrierKernel::gpu` with `Node::Pass` and
  `Resample`, `GpuCtx.res_scale`; there is no CPU kernel to mirror (`compute-kernels-render`): a kind is its passport,
  its WGSL and its stored frames (t328, t329 cycles).
- **Presets**: `src/apps/kt-app/assets/fx-presets-factory.json` (version 1) holds "Dream blur", "Old
  film", "Punch in" (a `geometry` zoom curve + `sharpen`), "Voice up front", "Wide dub echo"; keys
  carry their segment shape (`AutoKey`, R.61), including `Zigzag { periods, amplitude, profile }`.
- **Reconnaissance**: F-FX-008 (`docs/features/fx/stylize/lens-housing-dirt-drops-flare.md`), the live
  re-check of F-FX-007's lens rows.

## Goals / Non-Goals

**Goals:**
- The four lens kinds as built-in plugins under the existing golden and registry cycles; the three camera presets.
- The housing and the dirt keep the stage rule (vignette, noise): measured in their own stage,
  resolution-invariant.

**Non-Goals:**
- Tracking a light for the flare; animated (sliding) drops — a drop that moves is a function of
  time and seed, left to a later variant; physical lens simulation (Real Lens Flares-style element
  stacks); media overlay plates (F-CMP-003).
- "Fisheye switched on by speed changes" (ktprompts TODO) — deferred to F-SPD-002/F-AUT-008.

## Decisions

### D1. Families: the housing beside `vignette`, dirt and flare beside `glow`, drops beside `lens_distort`
A family crate depends only on kt-plugin-api and cannot borrow another family's code. So the housing goes to
`fx-video-color` (one radial-mask helper with `vignette`), dirt and flare to `fx-video-blur` with `glow` (the threshold
pass), drops to `fx-video-distort`.

### D2. Nothing waits
All three contracts exist, so the four kinds and the presets are independent commits. The presets need `lens_housing`
for "Fisheye lens" only.

### D3. Dirt multiplies the highlights, it never paints the frame
`out = in + k · H(in) · D(cell, seed)`, where `H` is glow's blurred-luma soft knee (threshold, radius
as glow's) and `D` a value-noise of the stage grid from the counter-based hash (two octaves, a
"smudge" scale and a "speck" scale). Below the threshold `H = 0` exactly, so the dark-frame bytes
stand (t1011). The pattern is pinned to the stage grid like `noise`; under the K× rule it is sampled
at the logical cell size, so 2× and 1× agree within `FrameTol`. Rejected: a texture file (ships
media, scales badly, F-FX-007 trick 3) and dirt added to the frame directly (shows on black).

### D4. Drops: cells, not a list
The warp has seven f64 numbers; a list of drops does not fit and would be a format of its own. `warp_lens_drops`
partitions the normalised space (`WarpNorm::ShortSemiAxis`) into `cells` per short axis; each cell holds at most one
drop whose centre offset, radius (× `size`, capped at one cell) and presence (`density`) come from the integer hash of
(cell x, cell y, seed) written once in the plugin's WGSL and once in its f64 `warp` (lowbias32, the same constants as
`tap_noise`). A point checks its own and the eight neighbouring cells. Inside a drop the source coordinate is
`c + (p − c) · (1 − refraction · (1 − (r/R)²))`. The seven numbers: cells, size, density, refraction, softness, the seed
(an exact f32 up to 9999) and one spare. Neutral (`refraction` 0) = no step. It fuses with a neighbouring `lens_distort`
in one pass because both are warps. `normalisation` is by the stage, so a later stage's zoom scales the drops with the
picture.

### D5. The flare: placed core, ghosts on the line through the centre
Position `(x, y)` in stage fractions (may be outside [0,1]), brightness, size, ghost count (≤ 8),
ghost spacing, tint. Ghost `i` sits at `c + (c − p) · s_i` with fixed `s_i` from spacing; each is a
soft disc added over the frame. A barrier with one pass (the per-pixel work is a short loop over the ghosts). Rejected: light detection (D2 of
F-FX-008 trick 3).

### D6. Presets as data
"Fisheye lens": `lens_distort` (k1 at its strong default) + `lens_housing` (defaults). "Zoom Shake":
`geometry` zoom 1.0 → 1.15 eased over 0.3 s, then a zigzag segment (periods 6, amplitude 0.03) back
to 1.0. "Handheld": `geometry` position x/y with long zigzag segments of small amplitude (periods 2
per second, amplitude 0.01 of the frame), offset phases on x and y. The factory file version stays 1;
"Punch in" is not touched (t1014 compares its record).

## Risks / Trade-offs

- [Drops at 30× minification alias] → the Jacobian supersampling of the fused pass covers a warp plugin's probe; t1012
  measures a minified case.
- [Dirt looks like noise on busy bright frames] → two scales and a coverage parameter; the default is
  tuned on our fixtures, not the owner's projects.

## Migration Plan

None: new plugin ids; old projects do not contain them; the preset file version stays 1.

## Open Questions

1. **Should drops slide?** Recommendation: not in W1 — a sliding drop is a function of time and seed
   that needs its own determinism case; a variant later.
2. **Which family owns a "fisheye" variant of `lens_distort` with the housing?** Recommendation: a
   chain preset (D6), not a variant — a variant is exactly one `FxInstance` (R.54), and this look is two.
