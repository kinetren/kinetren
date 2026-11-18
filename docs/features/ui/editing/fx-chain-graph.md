---
id: F-UI-008
title: The effect chain as a compact graph — picture and sound rails, paired devices, layers and the track stage
status: candidate
priority: B
cost: L
category: ui/editing
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, nuke-17, ableton-live-12, bitwig-6, reaper-7]
we_have: partial
crates: [kt-app, kt-model]
invariants: [3, 12]
ui_rules: [9, 13, 17]
round: null
point: change fx-graph
tests: []
depends_on: []
trackers: []
services: []
checked: 2026-11-18
---

# F-UI-008 — The effect chain as a compact graph — picture and sound rails, paired devices, layers and the track stage

## Essence

The person sees, in the window they already opened, WHAT happens to an event and IN WHICH ORDER:
its own picture and sound chains, the halves of a simulated device tied to each other, the morpher
layers that reach it, and the track stage after them. Each effect is a small node; a click selects
it and its existing form opens below. The owner's words (kkt-26.11.18b item 3): the morpher's and
the FX chain's control "дебильное", "нужно что-то типо компактного удобного графа".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | Video/Audio Event FX window | a horizontal row of "plug-in chain" buttons above the selected plug-in's form; video and audio are TWO windows; Pan/Crop leads the chain; an Adjustment Event's chain is the same row on another event |
| Premiere | 26 | Effect Controls | a vertical list of collapsible effects, fixed intrinsics (Motion, Opacity, Time Remapping, Volume) on top; an adjustment layer is a separate clip with its own list; video and audio in one panel, two sections |
| Resolve | 21 | Color page node editor; Fusion page | Color: compact nodes (number, label, a thumbnail on demand), serial/parallel/layer mixers, the order is the wire; Fusion: a free node graph with "pipe routers"; Edit page: Inspector — a list like Premiere |
| Final Cut Pro | 12 | Inspector | a vertical list of effects with a checkbox each; no routing view; audio effects in their own tab |
| After Effects | 2026 | Effect Controls + Flowchart | Effect Controls is a list; the Flowchart view draws compositions, layers and effects as a graph, read-only, rarely used |

**Outside the core.** The owner's request is a GRAPH of a chain that also carries modulation and
routing; the strongest compact forms of exactly this were born in DAWs and in Nuke, and three core
lines are "a list".

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Nuke | 17 | Node Graph | everything is a node; "postage stamps" optional; Dot nodes bend wires; the Properties bin opens the form of the selected node, not inside the graph |
| Ableton Live | 12 | Device View, Racks | a row of devices; a Rack folds parallel chains into ONE device with a chain list and eight macros; a folded device is a narrow strip with its title |
| Bitwig Studio | 6 | Device chain, containers, modulators | a row of devices; containers (FX Layer, Chain) nest rows inside a device; a modulator sits ON its device, its targets are lit only while it is selected |
| REAPER | 7 | FX chain, pin connector, containers | a list of slots and the selected plug-in's form beside it; routing of channels is a separate per-slot matrix; containers since 7.0 |

## The trick behind it

1. **The graph that works is a ROW, not a canvas.** Ableton, Bitwig and Resolve's Color page all lay
   the nodes out by ORDER, not by free placement: the person never arranges nodes and the layout is
   never stored. Free placement (Fusion, Nuke) pays only where the routing is arbitrary; where the
   engine has a fixed order, a canvas is a second place for the order to live and drift.
2. **A relation is drawn only while it matters.** Bitwig draws a modulator's routing only while the
   modulator is selected; REAPER keeps the pin matrix out of the list until asked. A permanent web of
   wires is what makes node graphs "spaghetti" — the compact ones show the TIE on demand and a small
   mark otherwise.
3. **Folding is what keeps it compact.** An Ableton Rack or a Bitwig container folds a whole sub-chain
   into one node with its title and a count, and unfolds in place. Resolve's compound node does the
   same. Without folding a graph is longer than the list it replaces.

## Pros

- The order of processing is visible: an event's chain, then the layers bottom-up, then the track —
  today the person must open three windows and know the rule to read it.
- A paired device is one thing on screen: the picture half and the sound half are two nodes with a
  tie, so "the sound of the tape" is a node with its own on/off, not a hidden derivation.
- No layout is stored: the graph is a pure function of the model, so it costs nothing in the file and
  cannot disagree with the render.

## Cons

- A row graph cannot express what the engine cannot do: no free routing, no parallel branch of an
  event's chain. A person used to Fusion will look for wires that do not exist.
- Two rails (picture and sound) take more height than one wrapped strip for a chain of one kind.
- Every window that shows a chain changes at once (rule 17 — one widget), and the layout tests that
  pinned the strip must be rewritten.

## What users say

NO QUOTES: searched (2026-11-18) the Vegas forum for FX chain and Plug-In Chooser complaints, Resolve
node-structure guides and Bitwig nesting reviews; the results were thread titles and summaries
(Vegas: effects cannot be dragged from the Plug-In Chooser onto clips, 2026; chain presets of Vegas 23
cannot be renamed) without a sentence verified on the page itself, so none is quoted. The owner's own
words in `docs/ktprompts/kkt-26.11.18b.txt` line 5 are the signal this note answers.

## How it fits Kinotrain

CAPABILITY: openspec/specs/ui-reach/

### What already exists

- ONE chain strip for events, morphers and tracks: `widgets::chain_strip` in
  `src/apps/kt-app/src/widgets.rs` (R.33 S2), with a pinned head (`HeadSlot`) and `ChainSel`.
- The FX window shows the anchor event and its link partners (`FxEditor.members`,
  `src/apps/kt-app/src/fx_editor.rs`); the morpher window merges both kinds into one lane
  (`merge_chains`/`split_chains`, `src/apps/kt-app/src/morpher_editor.rs`, R.35 item 3); the track
  window holds the track chain and the key track's compressor (`src/apps/kt-app/src/track_window.rs`).
- The device pair: `sync_pairs_to_link_group` (`src/libs/kt-model/src/model.rs`) for events,
  `MorphPatch::derived_pairs` (`src/libs/kt-model/src/project.rs`) for morpher patches.

### What is missing

- A morpher patch's paired half is DERIVED, never a slot: the window does not show it, and a
  bypassed sound slot added by hand does not stop the derivation (`derived_pairs` counts only enabled
  slots) — the owner's "нет возможности отключить".
- No view shows an event's whole order: its chain, the layers that reach it and the track stage.

### What hinders

The strip is one `horizontal_wrapped` row with indices into one `fx` vector; the graph needs the
per-rail index mapping that `merge_chains` keeps today only for the morpher. The R.35 decision "one
lane" for the morpher is the owner's and is revised only on his word.

### Which invariants it touches

3 (an edit through the graph is the same `SetFx`/`SetMorpher`; FX state is kept by clip id), 12 (the
graph repaints only while something moves); kt-app rules 9 (no second switch of one meaning), 13
(space is dear), 17 (paired ways equal, one widget).

## Estimate

| What | Answer |
|---|---|
| Scope | L — the model fix, the widget, three windows |
| Where the main work is | kt-app (the widget, three windows), kt-model (the patch's pair rule) |
| Project format | does not change the version; a morpher patch holding a paired kind gains its sound slot on the next save |
| Tests that will have to be written | the patch pair rule; the graph's layout and gestures; idle cost with the graph open; live scenario |
| Depends on | — |
| Breaks compatibility | no |

## Verdict

Take: a row graph of two rails with ties, folding groups and no stored layout; the forms stay. The
free canvas of Fusion/Nuke is not taken — the engine's order is fixed and a canvas would be a second
place for it. Design: `openspec/changes/fx-graph/design.md`.

## Sources

- https://www.bitwig.com/userguide/latest/advanced_device_concepts (2026-11-18, documentation)
- https://www.vegascreativesoftware.info/us/forum/cannot-drag-and-drop-video-effects-plugins-from-the-manager-onto-clips--150853 (2026-11-18)
- https://www.vegascreativesoftware.info/us/forum/fx-plugin-chooser-presets-can-t-be-deleted-or-renamed-vegas-23--150526 (2026-11-18)
- https://mixinglight.com/color-grading-tutorials/davinci-resolve-parallel-layer-mixer-nodes (2026-11-18)
- Ableton Rack, REAPER pin connector, Nuke Node Graph, AE Flowchart: by documentation and experience, not checked live
