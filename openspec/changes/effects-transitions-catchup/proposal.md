# Proposal

## Why

The owner (2026-10-27, item 6): "we are slipping badly on effects and transitions, far behind
CapCut and the like. We need to catch up" — and (item 7) the plugin system goes first, so the
catch-up is built as plugin crates (`plugin-api-stage1`). His claims file adds concrete looks:
fisheye with lens corners, dirt/drops/flares on the lens, letters as physical objects, a "67 niche
plugins" reference, homothety, bump mapping, "buffer + blend levels", audio-driven zoom. Without a
measured comparison the catch-up would be a wish list and would collide with rejections already
written in the video-fx design (a directional blur, sepia/invert as kinds).

## What Changes

- A reconnaissance note **F-FX-007** (`docs/features/fx/catalogue/market-catalogue-and-waves.md`):
  CapCut (the owner's yardstick, a new outside-the-core key `capcut-2026` in `_editors.md`) and the
  five core editors against our catalogue listed from the code (46 effect kinds, 33 transitions,
  15 generators), grouped by what people use (text animations, lens/glitch/light leaks, speed
  ramps, masks, beat sync, shake/zoom, blur/bokeh, retro/film, overlays, temporal looks, voice
  presets), each group with a verdict.
- **A ranked wave list** (W1 lens and camera → W2 glitch/film/soft → W5 transitions → W3 text
  animation library → W7 voice presets → W4 speed-ramp presets and beat sync → W6 temporal and
  relief), each wave a FUTURE change built as plugin family crates; the design gives each wave's
  kinds, variants, the new plugin declarations it needs and the earlier decisions it revises.
- **Guards** that keep the catalogue honest: the note lists exactly the built-in kinds the code
  has; the wave list names only existing note ids and capabilities and is ordered without forward
  dependencies; every row carries a verdict from a closed vocabulary; a wave that brings back a
  rejected idea carries an explicit "Revision:"; every FX item of the owner's claims file has a
  row.
- No effect, transition or generator is implemented by this change.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-fx`: adds the requirement that the video effect catalogue is measured against the market
  in a feature note that lists every built-in kind, and grows only through ranked waves built as
  plugin crates; a wave reviving a rejected kind is an explicit revision.
- `transitions`: adds the same rule for the transition vocabulary (the 33 kinds and the W5 gaps).
- `audio-fx`: adds the rule for audio effects (the voice-preset wave W7: presets over existing
  nodes before new nodes).

## Impact

- Docs: new `docs/features/fx/catalogue/market-catalogue-and-waves.md` (F-FX-007, new subcategory
  `fx/catalogue/`); `docs/features/_editors.md` gains the row `capcut-2026`;
  `docs/features/README.md` needs the `fx/` coverage row 6 / 6 → 7 / 7 and a summary-table row
  (left to the coordinator, as the round's rules say).
- Code: tests only — a new `src/apps/kt-cli/tests/fx_catalogue.rs` (reads the note and the
  registries; after `plugin-api-stage1`, the built-in plugin list).
- Depends on `plugin-api-stage1` (the waves are plugin crates; the guard reads its closed list).
- Tests reserved: t978–t983.
