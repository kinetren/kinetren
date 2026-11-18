# Proposal

> PLANNING ONLY. The owner sees this design first; code starts on his word ("discussion is not a
> go"). Revision 2 after his answers (U7). The questions still his are in `design.md`, "Questions for
> the owner", each with his words, today's state and a small mockup per option.

## Why

The owner, kkt-26.11.18b item 3 (registry row U3): "когда морфером добавляю датамош / vhstape / isp
- нет возможности отключить морфинг эффекта для аудио - там как будто у нас в морфере и впринципе в
fx-chain управление дебильное получается, нужно что-то типо компактного удобного графа делать"
(when I add Datamosh / VHS Tape / ISP with a morpher there is no way to turn off the effect's morph
for the audio; the control in the morpher and in the FX chain in general is clumsy; something like a
compact, convenient graph is needed).

Two things are inside it:

1. **A defect.** A morpher patch never holds the sound half of a simulated device: `MorphPatch::
   derived_pairs` (`src/libs/kt-model/src/project.rs`) DERIVES it in `chain_for`/`auto_for` from
   every enabled paired slot of the other kind. The morpher window shows only `patch.video_fx` +
   `patch.audio_fx` (`merge_chains`), so the derived half is invisible and has no checkbox; and a
   sound slot the person adds by hand and switches off does not stop the derivation, because
   `derived_pairs` counts only ENABLED slots of its own side — the switched-off half is derived
   again. On an event the same device is two real slots (`sync_pairs_to_link_group`), each with its
   own checkbox; in a morpher it is one slot plus a ghost. The mechanism, as a class: **a slot that
   exists for the render but not in the model the window edits** — walked in `design.md` D1.
2. **A request.** The chain strip (`widgets::chain_strip`) is one wrapped row of crumbs per window;
   the order an event is processed in (its chain → the morpher layers bottom-up → the track stage,
   the sidechain key) is visible nowhere at once, and a paired device reads as two unrelated crumbs in
   two places.

## What Changes

- **The morpher patch holds both halves of a device as real slots** (stage a): a paired kind put on
  one side of a patch puts its counterpart on the other side in the same `SetMorpher` (one undo
  step), values and curves shared, presence shared, the bypass of each half its own — the same rule
  as `sync_pairs_to_link_group` for events, one text for both. `derived_pairs` is removed;
  `Project::normalize` materializes the halves of patches read from older files. The sound half's
  switch-off keeps the device state: the halves share rows and timeline time, nothing else
  (sim-audio-pairs D3), so the picture is byte for byte the same with its sound half on or off.
- **A node editor replaces the chain strip** (owner, U7: "Вместо полоски", "везде в окнах разных
  единым стилем в одном месте чтобы не скролить … типо как в blender"), in every window that edits
  effects — an event and its link partners, a track, a morpher layer, a composition's instance. Not a
  new window (R.63). Blender-style: it opens framed and never scrolls (Home fits, wheel zooms, nodes
  collapse to headers), picture and sound are two socket kinds, a simulated device is ONE node with a
  picture lane and a sound lane each with its own switch, morpher layers / compositions / track
  stages are group nodes (Tab enters), a sidechain key is a wire into the compressor's key socket.
  The selected node's EXISTING form opens beside the graph. Effects are added by dragging a wire into
  empty canvas (the picker opens there) or Shift+A; M mutes, X removes, Ctrl+Shift+click shows the
  picture after a node in the preview (the Viewer).
- **Staged.** v1 = the editor over today's model: it shows and edits the straight chains the engine
  runs, and REFUSES a branch with a reason. v2 (a separate change) = branches, merges, mask and key
  inputs that `video_plan` and the audio node chain execute through the one plan (invariant 7), with
  slot ids and edges in the `.kt`. v3 is only a horizon (the timeline composite as a graph).
- **Cost at rest**: laid out once per (revision, size, zoom, pan, selection, collapsed set); hover
  re-tints; zoom/pan repaint only while moving; nothing asks a frame at rest.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `morpher`: a patch holds both halves of a paired device; the one-lane head-pinned strip becomes the
  node editor with the purple source-swap node first.
- `audio-fx`: "a simulated device is one across an A/V link group" — the morpher clause changes from
  derivation to real slots.
- `ui-reach`: the node editor — one widget for every chain, framed without scrolling, its sockets,
  devices, groups, gestures, refusals and its cost at rest.

## Impact

- Code: `src/libs/kt-model/src/project.rs` (`derived_pairs` gone; `chain_for`/`auto_for` simplified),
  `src/libs/kt-model/src/model.rs` (`sync_pairs_in_patch` beside `sync_pairs_to_link_group`, called
  from `SetMorpher`), `Project::normalize`; kt-app: a new `node_editor.rs` widget replacing
  `widgets::chain_strip`, `fx_editor.rs`, `morpher_editor.rs` (`merge_chains`/`split_chains` become the
  lane mapping), `track_window.rs`; i18n ×5 for the few new labels and hints.
- Project format: no version bump. A patch carrying a paired kind gains the counterpart slot in its
  other chain on the next save (a canonicalization, as `migrate_gen_to_events`). The graph stores
  nothing. OTIO: no new loss — morphers already travel only in `metadata["kt"]`; the sim-audio-pairs
  row of "What OTIO export loses" gains "and the halves held by a morpher patch".
- Tests reserved: t2990–t3009 (shared notes `fx-graph.md`). Rewritten by his answer "Вместо полоски": t244 and t218 (the
  morpher's strip assertions), same claims on the node editor.
- Feature note: `docs/features/ui/editing/fx-chain-graph.md` (F-UI-008).
