---
id: F-UI-008
title: Effects as a node graph — one editor in every window, picture and sound sockets, room for compositing
status: in-progress
priority: B
cost: XL
category: ui/editing
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2, nuke-17, natron-2.5, houdini-21, ableton-live-12, bitwig-6, reaper-7]
we_have: partial
crates: [kt-app, kt-model, kt-engine, kt-fx]
invariants: [3, 7, 11, 12]
ui_rules: [9, 13, 17]
round: null
point: change fx-graph (v1), fx-graph-dag (v2)
tests: [t2990, t2991, t2992, t2993, t2994, t2995, t2998, t2999, t3001, t3003]
depends_on: []
trackers: []
services: []
checked: 2026-11-20
---

# F-UI-008 — Effects as a node graph — one editor in every window, picture and sound sockets, room for compositing

## Essence

Every place that edits effects (an event, a track, a morpher layer, a composition) shows them as ONE
kind of node editor, in the same style, fitting its window without scrolling: nodes joined by wires,
picture and sound as two kinds of socket, a simulated device as one node carrying both, layers and
compositions as groups. First it shows and edits what the engine already does (straight chains);
later the same editor carries branches and merges the engine learns to execute. The owner's words
(kkt-26.11.18b items 3, U7): "компактного удобного графа", "везде в окнах разных единым стилем в
одном месте чтобы не скролить … графовое представление эффектов? типо как в blender?".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | Video/Audio Event FX window | a row of "plug-in chain" buttons over the selected plug-in's form; video and audio are two windows; no graph |
| Premiere | 26 | Effect Controls | a vertical list of collapsible effects; an adjustment layer is another clip with its own list; no graph |
| Resolve | 21 | Color nodes; Fusion page | Color: small numbered nodes, serial/parallel/layer mixers, the wire is the order; Fusion: a free node graph (tiles, pipe routers, groups, macros) for compositing; Edit page: a list |
| Final Cut Pro | 12 | Inspector | a list with a checkbox each; no graph |
| After Effects | 2026 | Effect Controls + Flowchart | a list; the Flowchart draws comps/layers/effects as a read-only graph nobody edits in |

**Outside the core.** Three of five core lines are "a list"; the node editors the owner names
(Blender) and the compositors he plans toward live outside video editing.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 | Compositor, Shader and Geometry Nodes | typed sockets by colour; Home frames all; H collapses a node to its header; Ctrl+G makes a group, Tab enters it; frames label regions; M mutes; Ctrl+Shift+click shows a node's output in the viewer; dragging a wire into empty space searches for a node to add there; dropping a node on a wire inserts it |
| Nuke | 17 | Node Graph | tiny nodes, F fits, Dot nodes bend wires, backdrops label regions, the form opens in the Properties bin, not in the node |
| Natron | 2.5 | Node Graph | the open-source Nuke-style graph: the same fit, dots and backdrops |
| Houdini | 21 | Networks, COPs/VOPs | networks nest (subnets, digital assets), network boxes, L lays the selection out automatically |
| Ableton Live | 12 | Racks | parallel chains folded into ONE device with macros |
| Bitwig Studio | 6 | containers, modulators | nested chains inside a device row; a modulator's routing is lit only while selected |
| REAPER | 7 | FX chain, pin connector | a list; routing is a separate matrix, opened on demand |

## The trick behind it

1. **What fits without scrolling is ZOOM plus COLLAPSE, not a smaller font.** Blender (Home, H),
   Nuke (F) and Houdini (L) all have one key that fits the network to the view and a way to fold a
   node to its header; the layout itself is rarely touched. A graph that cannot fit-and-fold is
   longer than the list it replaced.
2. **The form is not in the node.** Nuke's Properties bin, Blender's sidebar (N), Fusion's Inspector:
   the node carries its name and at most a few fields; the full form opens BESIDE the graph for the
   selected node. A node that carries its whole form is what makes graphs sprawl.
3. **Groups are the scale.** Blender's node group (Tab in, Tab out), Houdini's subnet and Fusion's
   macro turn a whole sub-graph into one node with sockets; the same group is used in many places.
   This is how a graph of hundreds of nodes stays one screen — and it maps one to one onto our
   morpher layers and compositions.
4. **The row graphs (Ableton, Bitwig, Resolve Color) win where the order is fixed**: the person never
   arranges nodes. Free placement pays only once the routing is free.
5. **A merge has a background, a foreground and a mask input — the background sets the size.**
   Nuke's Merge ("A over B": B is the background, the output keeps B's format, a `mask` input and a
   `mix` knob on every node), Fusion's Merge (Background, Foreground, Effect Mask) and Resolve
   Color's key input on a node and its Parallel/Layer mixer nodes all split the same three roles.
   Blender's Mix node takes a Fac socket for the same per-pixel weight. The non-obvious detail is
   the mask input: "blur only the highlights" is not a special effect but a merge whose mask is a
   key of the picture itself. Resolve's Layer Mixer gives the lower input priority — an order rule
   the person must learn; naming the roles avoids it. None of the NLEs (Vegas, Premiere, FCP) branch
   inside an event: they stack tracks with blend modes or track mattes, which is our timeline
   composite — the Merge law of `fx-graph-dag` (one event with a Merge = two stacked layers) ties
   the two together.

## Pros

- One editor and one look in every window that edits effects (rule 17), no strip that wraps and
  scrolls.
- The order of processing is visible: an event's chain, the layers over it, its track.
- A device is one node with a picture lane and a sound lane, each with its own switch.
- The same editor carries compositing later (several inputs, masks, keys, merges) without a second
  UI — the stage the engine is not ready for is shown as not available, not drawn as a promise.

## Cons

- A node editor over a linear engine draws wires the person cannot rewire yet: v1 must refuse a
  branch honestly until the engine executes one.
- A real DAG in the engine touches the plan builder, the GPU executor, the audio node chain and the
  project format — a round of its own, not a widget.
- Every window that shows a chain changes at once, and the layout tests that pinned the strip are
  rewritten.

## What users say

NO QUOTES: searched (2026-11-18) the Vegas forum for FX chain and Plug-In Chooser complaints, Resolve
node-structure guides and Bitwig nesting reviews; the results were thread titles and summaries
(Vegas: effects cannot be dragged from the Plug-In Chooser onto clips, 2026; chain presets of Vegas 23
cannot be renamed) without a sentence verified on the page itself, so none is quoted. The owner's own
words in `docs/ktprompts/kkt-26.11.18b.txt` (lines 5 and 21–24) are the signal this note answers.

## How it fits Kinotrain

CAPABILITY: openspec/specs/ui-reach/

### What already exists

- ONE chain strip for events, morphers and tracks: `widgets::chain_strip` in
  `src/apps/kt-app/src/widgets.rs`; the FX window holds the event and its link partners
  (`src/apps/kt-app/src/fx_editor.rs`); the morpher merges both kinds into one lane
  (`src/apps/kt-app/src/morpher_editor.rs`); the track window (`src/apps/kt-app/src/track_window.rs`).
- The engine executes linear chains (`kt_fx::plan::plan_chain`), layers back to front
  (`PlanLayer` in `src/libs/kt-engine/src/video_plan.rs`), morpher layers as appended slots with a
  strength slot, tracks and reels as nested layers; audio as a linear node chain (`AudioNode`).
- Several inputs exist already in special places: a transition pair, a stream stage's second input,
  masks, the sidechain key.

### What is missing

- A morpher patch's paired sound half is derived, not a slot — no switch (`MorphPatch::derived_pairs`).
- Any view of the whole order, and any editing of a non-linear graph.

### What hinders

The model is `fx: Vec<FxInstance>` with parallel `fx_auto`/`fx_bypass`/mask vectors addressed by
INDEX. v2 (`fx-graph-dag`) keeps the index: a slot's inputs may name only EARLIER slots, so the slot
order is a processing order, a line needs no new field, and no machine that addresses slots by index
changes. Free placement and wires across chains would need stable ids; neither is planned.

### Which invariants it touches

v1: 3 (edits are the same commands), 12 (no repaint at rest). v2: 7 (the DAG is built by the one plan
builder and run by the one executor), 11 (a linear graph renders byte for byte as the chain); kt-app
rules 9, 13, 17.

## Estimate

| What | Answer |
|---|---|
| Scope | v1 L (the editor over today's model); v2 XL (a DAG the engine executes, a format change) |
| Where the main work is | v1: kt-app, kt-model (the patch's pair rule); v2: kt-model, kt-engine, kt-fx, kt-gpu |
| Project format | v1 none (layout computed); v2 slot ids and edges, a version by content |
| Tests that will have to be written | the pair rule; the editor's gestures and fit; idle cost; v2: a line graph == the chain byte for byte |
| Depends on | — |
| Breaks compatibility | no |

## Verdict

Take, in two stages: v1 a Blender-style node editor (fit, collapse, groups, mute, viewer, link-drag
add) that shows and edits today's chains with picture and sound sockets; v2 branches and merges the
engine executes through the one plan. Design: `openspec/changes/fx-graph/design.md`.

## Sources

- https://www.bitwig.com/userguide/latest/advanced_device_concepts (2026-11-18, documentation)
- https://www.vegascreativesoftware.info/us/forum/cannot-drag-and-drop-video-effects-plugins-from-the-manager-onto-clips--150853 (2026-11-18)
- https://www.vegascreativesoftware.info/us/forum/fx-plugin-chooser-presets-can-t-be-deleted-or-renamed-vegas-23--150526 (2026-11-18)
- https://mixinglight.com/color-grading-tutorials/davinci-resolve-parallel-layer-mixer-nodes (2026-11-18)
- Blender node editor, Nuke/Natron Node Graph, Houdini networks, Fusion, Ableton Rack, REAPER pin connector, AE Flowchart: by documentation and experience, not checked live
