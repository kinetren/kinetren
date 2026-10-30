# Proposal

## Why

Wave W1 of the effects catch-up (F-FX-007, change `effects-transitions-catchup`, design D2/D3): the
owner's own first FX item — with the lens effect as a fisheye "you cannot put the lens corners
there", and "dirt/drops/flares on the lens" cannot be put on at all, "and they give their own
effects" (ktprompts, FX 1, translated). The whole lens group is missing: we have `lens_distort` and
nothing that a real lens adds besides its distortion. The live re-check of 2026-10-30 (F-FX-008)
confirmed the gap and the forms the market uses.

## What Changes

- Four new video kinds as built-in plugins:
  - `lens_housing` (point-wise, `src/plugins/fx-video-color` next to `vignette`): the dark round
    housing and soft corners of a fisheye, measured in its own stage's coordinates;
  - `lens_dirt` (barrier, `src/plugins/fx-video-stylize` next to `glow`): the frame's own highlights
    (glow's threshold) × a seeded procedural dirt pattern, added back — invisible on a dark frame;
  - `lens_flare` (barrier, `src/plugins/fx-video-stylize`): ghosts along the line from a placed,
    animatable light position through the frame centre;
  - `lens_drops` (geometry, `src/plugins/fx-video-distort`): refraction by seeded drops as a new
    `drops` MODE of the Warp primitive — the one core edit of the wave (`geom.rs` +
    `geom_fused.wgsl` in one commit).
- Three factory chain presets: "Fisheye lens" (`lens_distort` + `lens_housing`), "Zoom Shake" and
  "Handheld" (`geometry` curves with zigzag segments, R.61); the existing "Punch in" stays as it is.
- The picker, the catalogs (×5) and F-FX-007 learn the four ids; F-FX-007's lens rows become `have`
  when the wave is archived.
- No change to the project format: new plugin ids and presets in the existing preset file.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-fx`: adds the lens group — the housing measured in its own stage, dirt driven by the frame's
  highlights, drops as a Warp mode edited as a pair, a placed flare — and the camera presets as chains
  over existing kinds.

## Impact

- Code: `src/plugins/fx-video-color`, `src/plugins/fx-video-stylize`, `src/plugins/fx-video-distort`
  (created by `plugin-api-stage1` stage 4, or by this change with only its new kinds if that stage has
  not reached them); `src/libs/kt-fx/src/geom.rs` + `src/libs/kt-gpu/src/shaders/geom_fused.wgsl`
  (the `drops` Warp mode); `src/libs/kt-plugins-builtin` (`BUILTIN_IDS` +4);
  `src/apps/kt-app/assets/fx-presets-factory.json`; kt-app catalogs and picker keywords.
- Depends on `plugin-api-stage1`'s three missing contracts (its task 4.1): a positioned tap
  (`lens_housing`), geometry primitives in the API (`lens_drops`), a barrier pass on the GPU
  (`lens_dirt`, `lens_flare`). The presets do not wait.
- Tests reserved: t1010–t1015.
