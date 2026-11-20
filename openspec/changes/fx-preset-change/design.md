# Design: fx-preset-change

> DESIGN ONLY, revision 2 (2026-11-20), base `5e7a8ea9`: the owner's A4. Revision 1 (base `756f41b5`) kept curves on
> a preset change; he chose the replacement.

## Context

- The picker builds a slot from the kind's template and a variant (`fx_registry.rs`: `FxVariant` from
  `UiVariant { name_key, sets }`, values set by name through `set_param`).
- The preset library (`FxPresetStore`, `user_store.rs`) holds CHAINS with curves (factory + user); a chain preset is
  inserted by `kt_model::ops::append_chain_block` from `node_editor.rs` (keys re-hung "input to input").
- `SetFx` replaces the chain; `SetFxAuto` the curves; the editor sends both under one gesture token for one undo
  step; a value change keeps the effect's state (invariant 3).
- The graph (fx-graph v1b, `node_editor::chain_graph`): one selected node (`ChainSel`), Delete removes it; v2
  (`fx-graph-dag`) adds branches and wires and inserts picker candidates after the selected node on its arm.

## D1. The list

For a slot of kind K: the template as the kind's name ("Blur"), the variants of K exactly as the picker lists them
(one function `variants_of(K)` for both — UI rule 17), then the library's presets that are a single slot of kind K,
then "Save as preset…".

## D2. Replacing the effect

`preset_slot(K, v)` = defaults(K) ⊕ template(K) ⊕ v.sets, with no curves (a variant), or the user preset's slot with
its curves. Applying: the slot's parameters and every one of its curves become `preset_slot`'s; the curves are
re-hung on the event's entry point as a chain preset's are. Kept: the slot's position, its bypass flag, its masks,
its identity (an audio processor gets `set_params`, not a new object). One gesture token: `SetFx` + `SetFxAuto` in
one undo step; Undo restores values and curves.

## D3. The reading

On draw the slot is compared with each listed preset: every row's stored value and every curve (keys after
re-hanging) equal → that name; else "(custom)". A slot with a curve equals only a user preset with the same curve.
Cost: a few dozen comparisons per drawn form.

## D4. A chain preset joins the chain

- Insertion point: after the selected node on its lane (on v2, on its arm), else at the end. `ops::insert_chain_block_at`
  shares the re-hanging with `append_chain_block`.
- The inserted nodes become the selection; the graph scrolls them into view; the status line says "N nodes added —
  Delete removes the selected".
- Nothing existing is replaced or removed. On v2 a chain preset of one lane arrives as a run on the arm; wiring it
  elsewhere is v2's ordinary drag of a wire.

## D5. Several nodes selected

`ChainEdit.selected: ChainSelSet` (ordered set of `ChainSel`; the form shows the last clicked). Gestures: click —
one; Ctrl+click — toggle; a drag starting on the empty canvas — a selection frame (nodes whose rectangle the frame
touches); Delete — every selected node in one undo step (a device's both halves, the model's rule); dragging a
selected node moves the whole selection keeping its order (a reorder, one step). The head and the Geometry node
cannot be selected into a delete (they have no cross today). The keys stay those of v1b (no new keys).

## D6. Class walk — "a choice made only at creation" and "an insertion that cannot be undone in part"

| Place | Today | Here |
|---|---|---|
| effect variant | only at creation | D1–D3 |
| chain preset | appended at the end, nodes then removed one by one | D4, D5 |
| transition kind | changeable (transition window) | — |
| generator kind / title preset | title presets yes; a kind is the media | outside |
| paste of a chain block (Ctrl+V on the graph) | appended | the same insertion point and selection as D4 |
| morpher patch chain | the same graph | the same widget |

## Rejected

- **Keeping curves on a preset change** (revision 1) — the owner chose the replacement; Undo returns them.
- **A chain preset replacing the chain** — the owner: join, then delete the extra with the mouse.
- **Re-creating the slot** — loses position, mask and state.
