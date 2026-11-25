# Proposal: voice-presets

> DESIGN ONLY (spec session, 2026-11-25). Wave W7 of F-FX-007 (`openspec/specs/video-fx/design.md` D2), registry
> `kkt-26.10.27` K7, which no change covered. Tests reserved: t4279–t4280.

## Why

The owner's K7 (`kkt-26.10.27`): voices are missing from the catalogue. Every voice but the robot is a chain of nodes we
have; the robot needs one new node.

## What Changes

- Factory chain presets for audio: Telephone, Megaphone, Radio, Chipmunk, Deep voice, Small hall, Robot.
- One new audio node `ring_mod` (carrier Hz, mix), in the modulation family.

## Capabilities

### Modified Capabilities

- `audio-fx`: the node and the presets.

## Impact

- `src/plugins/fx-audio-modulation`, `src/apps/kt-app/assets/fx-presets-factory.json`, `BUILTIN_IDS` +1, catalogs ×5.
  No format change. Depends on nothing open.
