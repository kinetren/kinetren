# Design: fx-preset-change

> DESIGN ONLY, revision 3 (2026-11-24), base `229c694b` (graph v2d is in). The owner's A4 decided it. Implementation-ready.

## D0. What revision 2 said and the code contradicts

1. **`ops::insert_chain_block_at` beside `append_chain_block`.** Since v2d every gesture of a graph is an `Op` of
   `node_editor::perform`, re-wired by slot address (`fx_strip`) and checked by the model's `check_lane`; `Op::Add { items, fx_in }`
   already takes a whole block with its wiring. A second insertion would bypass the check. No model function is added.
2. **"After the selected node, at the end when none is selected."** v2d fixed the place: after the selected node on a lane that
   BRANCHES, else at the lane's end (`add_after`, `node_editor.rs:274`; the picker's cells show the same place). On a line the
   end stays: a node is nearly always selected there, and a chain preset that landed second in a chain of eight would not be
   "joined to the existing chain" but put inside it.
3. **`ChainEdit.selected` becomes a set.** 25 readers hold one `ChainSel` (the form, the head, the gallery, three windows). The
   form shows ONE node; a group is extra to it (D5).
4. **The Preset menu's reading over "values and curves".** The variants set rows by name through `set_param`, clamped; a plugin row
   stored as f32 gives 0.8 back as 0.800000012 (input-without-work): the comparison is of STORED values, `FxInstance == FxInstance`
   of two instances built by the same `var()`, never of the picker's numbers.
5. **Morpher and track**: a variant there has no masks and, on a track, curves only where allowed (`TrackCurvesUnsupported`): D2
   names what each owner writes.
6. **A drag on the empty canvas** is free (the pan is the middle button, a double click fits, Ctrl+Shift+click ends the Viewer);
   but below the graph a dropped WIRE starts a branch (`BRANCH_ZONE`): a wire drag begins on a socket, so the two do not meet.
7. **Ctrl+click on a node** is free (Ctrl+Shift is the Viewer's).
8. **A device's two halves** (VHS picture and sound) are one device: a preset of one half is applied to its pair where the model
   mirrors (`mirror_halves`), or the halves part.

## Context

`node_editor.rs` (`perform :317`, `Op :243`, keys `GraphKey::Remove :529`, nodes `:1072–1200`), `widgets::{ChainEdit :2111,
ChainSel :2205, ChainChanged}`, `fx_registry.rs` (`registry()`, `FxVariant :43`, `var`), `user_store::{FxPresetStore, PresetRow :45}`
(blocks of slots with curves and masks, keys from zero), `fx_editor.rs` (the slot form `:1837`, its commit), `morpher_editor.rs`,
`track_window.rs`. Cells and the form's door are one popup (`fx_picker_at`).

## D1. The list

`fx_registry::presets_of(f: &FxInstance, store: &FxPresetStore) -> Vec<PresetChoice>`: the kind's template, its variants as the
picker shows them (the same `FxEntry`), then the library's rows whose block is ONE slot of this kind, then "Save as preset…"
(the window of today with this one slot). The picker's gallery reads the same entries; a new kind gets its menu for free.

## D2. Replacing the effect

`apply_preset(edit, i, choice)`: the slot's `FxInstance` = the choice's; its curves = none (template, variant) or the preset's
re-hung on the owner's entry (`rebase_curves`, the rule of `append_chain_block`). Kept: the place, the wiring, the bypass, the
masks (a single-slot preset's own masks are NOT taken: "заменяется" is about the effect; a mask is the person's drawing on this
event), the Merge mode, `ChainChanged::from` (none: nothing moved), so an audio effect is `set_params` and keeps its tail (t1940's
diff by place and kind). A stream kind's structural row changed restarts its history as any edit of it does.

| Owner | What it sends |
|---|---|
| event | `ch.fx`, `ch.auto` → its session's `SetFx` + `SetFxAuto` under one gesture token of THIS choice (not the window's, so one Undo returns it) |
| morpher | `SetMorpher` with the same token rule |
| track | `SetTrackFx`; a preset with a curve the track does not take is listed disabled with its reason |
| device halves | both halves in the same step where the model mirrors |

## D3. The reading

On draw: the first choice with `choice.fx == slot.fx && choice.curves == slot.curves` (after re-hanging), else "(custom)"; kept
per (slot, its value) in the form's memory, so a hover compares nothing. Picking the shown one writes nothing (the model's
input-without-work).

## D4. A chain preset joins the chain as a group

`Op::Add`/`Op::Branch` already insert the block; `perform` marks what it added: `ChainEdit.group` = the new slots by their places
after `settle`, the form shows the last, the canvas scrolls to it, the status line "5 nodes added — Delete removes the selected".
A paste (Ctrl+V, the bar's button) is the same `Op::Add`. Joining them elsewhere is v2d's wire drag.

## D5. Several nodes

`ChainEdit.group: &mut BTreeSet<usize>` beside `selected` (empty or containing the selected slot). A plain click: one; Ctrl+click:
toggle (the Lead and the head cannot join); a left drag that starts on the canvas outside every node, wire and socket: a frame, the
nodes it touches, Ctrl adds; Esc or a click on the empty canvas: none (Esc before the window's, as the Viewer's step).
- Delete, the cross of a selected node and "Remove" in its menu: `Op::RemoveMany(set)` in one step (device halves by the model).
- A drag of a selected node's name: `Op::ShiftMany(set, To)`: their order kept, the place as for one. Nodes of two arms: refused with
  the words (rule 16), nothing changes.
- Mute (its key and the checkbox with Ctrl): all of them. The Viewer, sockets and the form stay one node's.
- `perform` renumbers the group through `Strip::from` as it does `select`; an owner's other edit that changes the slots drops it.
- Drawing: the group's nodes have the selection stroke, the form's node a thicker one. Hover lays nothing out (t3221).

## D6. Class walk: "a choice made only at creation" and "an insertion that cannot be undone in part"

| Place | Today | Here |
|---|---|---|
| effect variant (event, morpher, track; graph v2 arm) | only in the picker | D1–D3 |
| chain preset, paste of a chain | appended; nodes removed one by one | D4, D5 |
| transition kind, its row presets | changeable in its window | — |
| title presets, generator variants | a menu that sets values and curves | — |
| camera moves | none | `presets-everywhere` |
| curve presets, param presets, transition presets | none | `presets-everywhere` (it takes D1's list function) |
| mask presets | none | not asked |
| audio device chain of a track | the same graph | the same widget |

## Idle

A comparison per change of a slot; nothing at rest.

## Rejected

- **Keeping curves on a preset change** — he chose the replacement.
- **A chain preset replacing the chain**, or arriving unwired — "к существующей цепочке … соединить".
- **Re-creating the slot** — loses place, mask, wires and state.
