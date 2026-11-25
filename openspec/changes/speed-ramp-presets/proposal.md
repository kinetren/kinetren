# Proposal: speed-ramp-presets

> DESIGN ONLY (spec session, 2026-11-25). Wave W4 (the ramps; beat and zoom by sound are `edit-analysis`) of F-FX-007 (`openspec/specs/video-fx/design.md` D2), registry
> `kkt-26.10.27` K7, which no change covered. Tests reserved: t4284–t4286.

## Why

The owner's K7: speed ramps are the first effect people look for on short videos, and ours are curves drawn key by key.

## What Changes

- Six ramps in the speed row's menu: Montage, Hero, Bullet, Jump Cut, Flash In, Flash Out. A click writes the speed curve
  over the event's length; the first and last frames of the event stay the same; the curve is editable after.

## Capabilities

### Modified Capabilities

- `automation`: the ramps.

## Impact

- `kt-model` `warp.rs` (a pure writer), `kt-app` `automation.rs` (a menu on the `rate` row of a warp slot), catalogs ×5. No
  kernel, no format change. Depends on nothing.
