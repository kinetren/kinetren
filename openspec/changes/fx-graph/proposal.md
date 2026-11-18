# Proposal

> PLANNING ONLY. The owner sees this design first; code starts on his word ("discussion is not a
> go"). The decisions that are his are listed in `design.md`, section "Open decisions (the owner's)".

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
- **A compact chain graph replaces the chain strip** in the three windows that edit a chain — the
  FX window (an event and its link partners), the morpher window, the track window (stage b, c).
  Not a new window (R.63 ban). Two rails, picture above and sound below, laid out by ORDER (no stored
  positions); a node is a chip with an on-dot and a short name; a paired device is two nodes with a
  tie; a morpher layer reaching the event is a folded purple group after the event's chain; the track
  stage and the sidechain key are the tail. Selecting a node opens its EXISTING form below (deck face,
  ISP panel, passport grid, automation widget) — the forms are not touched.
- **Gestures**: click = select (form below); click the on-dot = bypass; drag along a rail = reorder;
  `+` at a rail's end or in a gap = the existing picker gallery inserting there; `×`/Delete = remove
  (a device: both halves); Ctrl+C/V = the existing slot/chain blocks; click a group = unfold in place;
  double click a layer group or the track tail = open that layer's/track's window.
- **Cost at rest**: the graph is laid out once per (model revision, window width, selection,
  fold state) and replayed; hover re-tints only; no animation; nothing asks a frame at rest.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `morpher`: a patch holds both halves of a paired device; the one-lane head-pinned strip becomes the
  graph with the pinned head on the picture rail.
- `audio-fx`: "a simulated device is one across an A/V link group" — the morpher clause changes from
  derivation to real slots.
- `ui-reach`: the chain graph — one widget for every chain, its gestures, its rails, and its cost at
  rest.

## Impact

- Code: `src/libs/kt-model/src/project.rs` (`derived_pairs` gone; `chain_for`/`auto_for` simplified),
  `src/libs/kt-model/src/model.rs` (`sync_pairs_in_patch` beside `sync_pairs_to_link_group`, called
  from `SetMorpher`), `Project::normalize`; kt-app: a new `fx_graph.rs` widget replacing
  `widgets::chain_strip`, `fx_editor.rs`, `morpher_editor.rs` (`merge_chains`/`split_chains` become the
  graph's rail mapping), `track_window.rs`; i18n ×5 for the few new labels and hints.
- Project format: no version bump. A patch carrying a paired kind gains the counterpart slot in its
  other chain on the next save (a canonicalization, as `migrate_gen_to_events`). The graph stores
  nothing. OTIO: no new loss — morphers already travel only in `metadata["kt"]`; the sim-audio-pairs
  row of "What OTIO export loses" gains "and the halves held by a morpher patch".
- Tests reserved: t2990–t3009 (shared notes `fx-graph.md`). Rewritten by decision: t244 and t218 (the
  morpher's "one lane" assertions) — only if the owner takes two rails (decision O2).
- Feature note: `docs/features/ui/editing/fx-chain-graph.md` (F-UI-008).
