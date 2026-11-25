# Proposal: title-motion-slots

> DESIGN ONLY (spec session, 2026-11-25). Wave W3 (the library half; words are `title-word-effects`) of F-FX-007 (`openspec/specs/video-fx/design.md` D2), registry
> `kkt-26.10.27` K7, which no change covered. Tests reserved: t4281–t4283.

## Why

The owner's K7 and v26.10 F2 ("the text generator is poor"): a title has five whole-title presets, each with at most one
animation, and no way to say "arrive like this, leave like that".

## What Changes

- A title's window gets three slots: In, Out and Loop. Each takes one motion from a list of ten (fade, slide from a side ×4, rise,
  scale in, type on, soft type, blur-in; Loop: pulse, sway, float) with a length; the curves are written on the title's
  own parameters and may be edited by hand after.
- Slots are curves and nothing else: no new field in the project, no format change.

## Capabilities

### Modified Capabilities

- `generators`: the motion library of a title.

## Impact

- `kt-model` `genmedia.rs` (`title_motion_curves`, beside `text_anim_curves`), `kt-app` `gen_ui.rs` (the title section), catalogs ×5.
- Depends on nothing. Beside `title-word-effects`: different files (it edits `kt-text`, MASKS).
