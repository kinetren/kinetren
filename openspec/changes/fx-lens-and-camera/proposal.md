# Proposal

> REVISION 2 (spec session, 2026-11-25, on 36d751a7). The first text waited for three contracts of
> `plugin-api-stage1`: all three are in (positioned taps, WGSL warp plugins, barrier graphs). Two facts changed since:
> `glow` lives in `fx-video-blur`, and kernels have no CPU twin (`compute-kernels-render`). Implementation-ready, may
> start now.

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
  - `lens_dirt` (barrier, `src/plugins/fx-video-blur` next to `glow`): the frame's own highlights
    (glow's threshold) × a seeded procedural dirt pattern, added back — invisible on a dark frame;
  - `lens_flare` (barrier, `src/plugins/fx-video-blur`): ghosts along the line from a placed,
    animatable light position through the frame centre;
  - `lens_drops` (geometry, `src/plugins/fx-video-distort`): refraction by seeded drops, a warp plugin with its own WGSL
    arm (`warp_lens_drops`), no core edit.
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

- Code: `src/plugins/fx-video-color` (housing), `src/plugins/fx-video-blur` (dirt, flare), `src/plugins/fx-video-distort`
  (drops); `src/libs/kt-plugins-builtin` (`BUILTIN_IDS` +4); `src/apps/kt-app/assets/fx-presets-factory.json`; kt-app
  catalogs and picker keywords. No core crate.
- Depends on nothing open.
- Tests reserved: t1010–t1015.
