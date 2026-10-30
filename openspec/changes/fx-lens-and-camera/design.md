# Design

## Context

See proposal.md — Why. What exists and shapes the plan (observed on 2026-10-30):

- **The lens we have**: `lens_distort` is Warp mode 0 (`GeomPrim::Warp { mode, p: [f64; 7] }`,
  `src/libs/kt-fx/src/geom.rs:503`; modes 0 lens, 1 twirl, 2 ripple, 3 kaleidoscope, 4 cylinder),
  mirrored in `src/libs/kt-gpu/src/shaders/geom_fused.wgsl`; tolerance class `FrameTol::WARP`.
- **The mechanisms the new kinds reuse**: `vignette` measures its radius in its own stage's
  coordinates; `glow` (`src/libs/kt-fx/src/video.rs:1279`) thresholds the BLURRED luma with a soft
  knee; `tap_noise` (`geom.rs:1865`) pins grain to its stage's grid with a counter-based hash, byte
  for byte on CPU and GPU.
- **The plugin API today** (`src/libs/kt-plugin-api/src/lib.rs`): `VideoKernel::tap` sees one
  pixel's VALUE (no coordinate); `Gpu` has no assembled barrier pass yet; geometry plugins must give
  host primitives (`register` refuses WGSL for them) but `GeomPrim` lives in kt-fx. These are the
  three gaps `plugin-api-stage1` task 4.1 records for moving `vignette`, `noise`, the geo kinds and the
  barriers (commit b5b1295). On this branch `BUILTIN_IDS` holds 19 ids; `fx-video-color` exists,
  `fx-video-stylize` and `fx-video-distort` do not yet.
- **Presets**: `src/apps/kt-app/assets/fx-presets-factory.json` (version 1) holds "Dream blur", "Old
  film", "Punch in" (a `geometry` zoom curve + `sharpen`), "Voice up front", "Wide dub echo"; keys
  carry their segment shape (`AutoKey`, R.61), including `Zigzag { periods, amplitude, profile }`.
- **Reconnaissance**: F-FX-008 (`docs/features/fx/stylize/lens-housing-dirt-drops-flare.md`), the live
  re-check of F-FX-007's lens rows.

## Goals / Non-Goals

**Goals:**
- The four lens kinds as built-in plugins with CPU references and WGSL mirrors, under the existing
  golden and conformance cycles; the three camera presets.
- The housing and the dirt keep the stage rule (vignette, noise): measured in their own stage,
  resolution-invariant.

**Non-Goals:**
- Tracking a light for the flare; animated (sliding) drops — a drop that moves is a function of
  time and seed, left to a later variant; physical lens simulation (Real Lens Flares-style element
  stacks); media overlay plates (F-CMP-003).
- "Fisheye switched on by speed changes" (ktprompts TODO) — deferred to F-SPD-002/F-AUT-008.

## Decisions

### D1. Families: the housing beside `vignette`, dirt and flare beside `glow`, drops beside `lens_distort`
A family crate depends only on kt-plugin-api (and kt-text for titles); it cannot borrow another
family's code. So the housing goes to `fx-video-color` where `vignette` will live (one radial-mask
helper), dirt and flare to `fx-video-stylize` with `glow` (one threshold-and-blur helper), drops to
`fx-video-distort` with `lens_distort`. This corrects D2 of `effects-transitions-catchup`, which
named only distort and color for W1. If `plugin-api-stage1` stage 4 has not created a family yet,
this change creates it holding only the new kinds; the moved kinds join it later without touching
the new ones.

### D2. The stages follow the plugin API contracts, presets first
1. Presets (no kernel, no dependency).
2. `lens_housing` — after the POSITIONED tap lands (`plugin-api-stage1` D2b/D5a: the tap receives its
   stage coordinate, as `vignette` needs).
3. `lens_dirt`, `lens_flare` — after the BARRIER pass lands (a fragment pass over `common.wgsl` with
   the 16-float uniform block, `plugin-api-stage1` D5).
4. `lens_drops` — after geometry primitives are reachable from the API; the `drops` mode itself is a
   core edit.
Rejected: making the four kinds enum variants now and moving them later — `plugin-api-stage1`
admits new kinds only as plugins, and a later move would be four golden migrations for nothing.

### D3. Dirt multiplies the highlights, it never paints the frame
`out = in + k · H(in) · D(cell, seed)`, where `H` is glow's blurred-luma soft knee (threshold, radius
as glow's) and `D` a value-noise of the stage grid from the counter-based hash (two octaves, a
"smudge" scale and a "speck" scale). Below the threshold `H = 0` exactly, so the dark-frame bytes
stand (t1011). The pattern is pinned to the stage grid like `noise`; under the K× rule it is sampled
at the logical cell size, so 2× and 1× agree within `FrameTol`. Rejected: a texture file (ships
media, scales badly, F-FX-007 trick 3) and dirt added to the frame directly (shows on black).

### D4. Drops: cells, not a list
The Warp uniform has seven f64 slots; a list of drops does not fit and would be a format of its own.
The `drops` mode partitions the normalised stage into `cells` per short axis; each cell holds at most
one drop whose centre offset, radius (× `size`) and presence (`density`) are a counter-based hash of
(cell, seed); a pixel checks its own and the eight neighbouring cells (the radius is capped at one
cell). Inside a drop the source coordinate is `c + (p − c) · (1 − refraction · (1 − (r/R)²))`
(a lens-like magnification; negative refraction flips toward the drop-on-the-lens look). Slots:
seed, cells, size, density, refraction, edge softness, aspect. CPU f64 and WGSL f32 of the same
formula; the hash is integer and identical. `drops` is mode 5, appended after cylinder.

### D5. The flare: placed core, ghosts on the line through the centre
Position `(x, y)` in stage fractions (may be outside [0,1]), brightness, size, ghost count (≤ 8),
ghost spacing, tint. Ghost `i` sits at `c + (c − p) · s_i` with fixed `s_i` from spacing; each is a
soft disc added over the frame. A barrier because the GPU side is a pass of its own (the per-pixel
work is a short loop over ghosts, a point-wise tap could hold it, but the positioned tap and the
ghost loop are cheaper as one fragment pass than as N taps). Rejected: light detection (D2 of
F-FX-008 trick 3).

### D6. Presets as data
"Fisheye lens": `lens_distort` (k1 at its strong default) + `lens_housing` (defaults). "Zoom Shake":
`geometry` zoom 1.0 → 1.15 eased over 0.3 s, then a zigzag segment (periods 6, amplitude 0.03) back
to 1.0. "Handheld": `geometry` position x/y with long zigzag segments of small amplitude (periods 2
per second, amplitude 0.01 of the frame), offset phases on x and y. The factory file version stays 1;
"Punch in" is not touched (t1014 compares its record).

## Risks / Trade-offs

- [The plugin API contracts slip] → stage 1 (presets) is independent; stages 2–4 each wait for one
  contract only, so a slip delays one kind, not the wave.
- [Drops at 30× minification alias] → the Jacobian supersampling of the fused pass already covers Warp
  modes; t1012 adds a minified case.
- [Dirt looks like noise on busy bright frames] → two scales and a coverage parameter; the default is
  tuned on our fixtures, not the owner's projects.

## Migration Plan

None: new plugin ids; old projects do not contain them; the preset file version stays 1.

## Open Questions

1. **Should drops slide?** Recommendation: not in W1 — a sliding drop is a function of time and seed
   that needs its own determinism case; a variant later.
2. **Which family owns a "fisheye" variant of `lens_distort` with the housing?** Recommendation: a
   chain preset (D6), not a variant — a variant is exactly one `FxInstance` (R.54), and this look is two.
