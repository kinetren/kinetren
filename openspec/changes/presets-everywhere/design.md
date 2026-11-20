# Design: presets-everywhere

> DESIGN ONLY, revision 1 (2026-11-20), base `ccae9d10`. Q1–Q2 in `proposal.md`; defaults recommended.

## Context

- `FxPresetStore` (`src/apps/kt-app/src/user_store.rs`): chain presets with curves, raw entries kept, read-only
  for a newer file, `.broken` on a parse failure, factory presets in the binary (capability `automation`,
  "Chain presets carry curves and live in user storage").
- The clipboard blocks `FxSlotBlock`, `FxChainBlock`, `EnvelopeBlock` (`src/libs/kt-model/src/ops.rs`): a paste
  re-hangs keys on the target's entry point; an envelope pastes only into its own kind.
- Keys' domains: an effect curve in the material tick (R.33), a transition curve from the window start, a camera
  by the event's progress (kt-model CLAUDE.md).
- Links (R.61), the slot Preset menu (change `fx-preset-change`), title presets (generators).

## D1. Entities and their blocks

| Entity | Block (clipboard and preset) | Key domain on paste |
|---|---|---|
| effect slot | `FxSlotBlock` (exists) | material, re-hung on entry |
| chain | `FxChainBlock` (exists) | material, re-hung |
| curve | `EnvelopeBlock` (exists) + a relative form for presets | material; a preset fits 0..1 of the target event |
| transition | `TransitionBlock { kind, values, auto, invert, audio }` (new) | window start |
| generator values | `GenBlock { kind, values, auto }` (new, only onto the same kind) | material |
| masks | `MaskBlock { masks }` (new) | owner's material |
| camera | `CameraBlock { params, auto }` (new) | event progress |
| morpher patch | `PatchBlock { patch }` (new) | the layer's own rule |
| geometry node | `GeomBlock { geometry, auto }` (new) | material |

Each block is serde with `skip_serializing_if` (the clipboard rule of kt-model) and has a paste function that
returns commands (one batch).

## D2. The preset store

`PresetStore { kind: PresetKind, … }` generalises `FxPresetStore`: one file per kind under the user store
(`presets/<kind>.json`), the same raw/parsed/read-only/broken rules, factory presets per kind in the binary.
The existing chain file is the `chain` kind (no migration of its text). A curve preset stores keys as `(u, v)`
with u = position in 0..1 of its span and v in the row's unit with the row's spec id; pasting onto a row of
another range rescales by the row specs (refused across units, named).

## D3. The head menu

`widgets::entity_head(ui, Entity)` — one widget: Copy, Paste (enabled when the clipboard holds this entity's
block), Presets ▸ (built-in, separator, mine), Save as preset…, Take from event…. Every entity form calls it
(the effect slot head of `fx-preset-change` becomes its slot form). A guard enumerates the entity forms (the
registry windows and the chain strip) and fails when one lacks the head.

## D4. Take from event…

1. The menu item enters a pick mode (status line: "click the event to take from — Esc cancels"); the timeline's
   hover outlines eligible sources (events, transitions, camera blocks, track headers).
2. A click opens the checklist window (a registry row: "a choice of WHAT to take cannot live in a menu"): the
   source's parts as checkboxes (picture chain, sound chain, their curves, geometry, masks, level/gain,
   generator values when the kinds match), "keys: keep timing / stretch to length", and for a single curve
   "Link instead" (Q1 a).
3. OK: `ops::take_from(source, targets, checklist)` → one batch over the target (or every selected event of a
   compatible kind); a part the target cannot take (a sound chain onto a video event) is skipped and counted in
   the status line, never applied half (the paste rule of `paste_block`).

## D5. Class walk — "an entity without copy or presets"

The table of D1 is the walk: every entity with values in the model, today's state and this change's block.
Not included: markers and areas (copied with blocks already), the project settings (a template is the welcome
window's), the render settings (the render window has presets).

## Rejected

- **Take-from as a live link for everything** (Q1 b) — contradicts the owner's C2c (copies independent).
- **A preset file for all kinds together** — one broken kind would lock all; one file per kind.
