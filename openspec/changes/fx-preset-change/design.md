# Design: fx-preset-change

> DESIGN ONLY, revision 1 (2026-11-20), base `756f41b5`.

## Context

- The picker builds a slot from the kind's template and a variant (`fx_registry.rs`: `FxVariant` from
  `UiVariant { name_key, sets }`, values set by name through `set_param`).
- The preset library (`FxPresetStore`, `user_store.rs`) holds CHAINS (factory + user), filtered per event
  kind in `fx_editor.rs`; the strip offers saving/inserting chains.
- `SetFx` replaces the chain; `SetFxAuto` the curves; the editor sends both under one gesture token for one undo
  step; identity of slots by position — a value change keeps the effect's state (invariant 3, "parameters →
  `set_params`").

## D1. The list

For a slot of kind K: the variants of K exactly as the picker lists them (one function `variants_of(K)` used by
both — UI rule 17), then the library's presets that are a single slot of kind K (their names), then "Save as
preset…" (the library's existing save). The template itself is listed first as the kind's name ("Blur").

## D2. Applying

`preset_values(K, v) = defaults(K) ⊕ template(K) ⊕ v.sets` (a user preset: its slot's values). The new slot =
the current slot with every row WITHOUT a curve set to `preset_values`; rows with a curve keep the curve (Q1 a);
flags and choice rows included. One `SetFx` (+ nothing else) — position, bypass, masks, morpher/graph wiring
untouched. Undo restores the former values.

## D3. The reading

On draw, the slot's rows without curves are compared with each listed preset's values (exact row-store
equality, the file's own casting); the first equal one is shown, else "(custom)". Picking the shown one is
allowed (a reset to it). Cost: a few dozen row comparisons per drawn form — nothing at rest.

## D4. Class walk — "a choice made only at creation"

| Choice at creation | Changeable after? | Here |
|---|---|---|
| effect variant | no | this change |
| transition kind | yes (transition window) | — |
| generator kind / title preset | title presets yes; generator kind no (a kind is the media) | outside: a kind change is a new source |
| morpher target mode | yes | — |
| track blend mode | yes | — |
| export preset | yes (render window) | — |

## Rejected

- **Re-creating the slot** (what the owner does by hand today) — loses position, mask and state.
