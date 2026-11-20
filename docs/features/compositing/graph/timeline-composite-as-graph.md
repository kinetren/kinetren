---
id: F-CMP-004
title: The timeline composite as a graph — tracks as Merge nodes, mattes across tracks, a Viewer anywhere
status: researched
priority: C
cost: XL
category: compositing/graph
editors: [vegas-pro-2026, resolve-21, nuke-17, after-effects-26]
we_have: partial
crates: [kt-model, kt-engine, kt-gpu, kt-app]
invariants: [4, 7, 8, 11, 12]
ui_rules: [9, 13]
round: null
point: null
tests: []
depends_on: [F-UI-008, F-CMP-003, F-TL-004]
trackers: []
services: []
checked: 2026-11-20
---

# F-CMP-004 — The timeline composite as a graph — tracks as Merge nodes, mattes across tracks, a Viewer anywhere

HORIZON: not planned. This is "v3" in the fx-graph-dag design (D5, "the composite as a graph").
The note records what other programs do, what in our engine would have to change, and the
questions only the owner can answer. Nothing here is a design.

## Essence

Today a frame is made in two steps:

- each event, track stage and reel instance runs its effect graph (v2 since fx-graph-dag);
- then the timeline composite folds the layers back to front in the Vegas order: level, fade,
  blend mode, transition pairs.

That fold is a fixed shape: a layer can only lie over everything below it. "v3" would make the fold
itself a graph:

- a track is a Merge node;
- a layer may take another track's picture as a matte or as a second input (a displacement, a key);
- a Viewer can show any node: the picture "below track 3", or one track alone with its matte.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Parent/child compositing ("Make Compositing Child") | a TREE: a parent track composites its children first, then applies its own mode and Parent Composite Mode; no input from a non-child track; masks via "Multiply (Mask)" on the parent |
| DaVinci Resolve (Fusion page) | 21 | Fusion clip; MediaIn / Merge / MediaOut | the Edit timeline stays layers; selected stacked clips become ONE Fusion clip whose stack is built as a cascade of Merges, each Merge with Background, Foreground and an effect mask input; any node can be viewed |
| Nuke | 17 | Merge (A over B), the node graph | the whole comp is a DAG; B is the stream (its format is the output's), A is laid over; bbox union/intersection/A/B; every node's output can be viewed and cached |
| After Effects | 26 | Track Matte (any layer, since 23.0), precomps, adjustment layers | the layer stack stays the order; since 23.0 a layer's matte may be ANY layer of the comp, not only the one above; a precomp is a subgraph with one output |

## The trick behind it

**1. Resolve did not turn its timeline into a graph: it lifts a selection out of the timeline into
one.** The Edit page stays layers. "New Fusion Clip" turns a stack into a cascade of Merge nodes, so
the graph's default IS the layer fold. This is our Merge law one level up: a stack of tracks with
modes equals a left fold of Merges, so a graph whose default is the fold costs nothing until someone
draws a wire. Resolve pays for keeping two worlds: the Fusion clip renders at the TIMELINE's
resolution, and the forums are full of "the Fusion clip is black in the timeline while MediaOut looks
right".

**2. Nuke's cache is per node and keyed like a Merkle tree.** Each node's hash is its knobs plus its
inputs' hashes. An edit to the top of a comp leaves every node below it with the same hash, so the
picture under the edited layer comes from the cache. Our keys hash a moment's whole plan at once
(`plan_key`, `durable_hash`), so ANY edit re-keys the whole frame. A graph composite with node keys
would give "edit the title, the background stays kept". Today that is impossible in principle.

**3. After Effects kept the stack and added only cross-edges, and the price was the default.** Since
23.0 any layer can be the matte of any layer. Users lost the fast "the layer above" and asked for it
back. Adobe added a "Matte with Layer Above" command, and scripts fall back to the layer above when
no matte layer is set. A matte layer is hidden by default: the cross-edge consumes its source. That
is a decision about what a track IS, not a rendering detail.

**4. A merge has a SIZE rule, and it must be one.** In Nuke, "B" sets the format, and the bbox of a
merge grows to the union unless told otherwise, "because Nuke does not know that the extra area is
going to be black". Our composite has one canvas (the project raster), and each layer is fitted into
it before the fold. v2's Merge already takes its Background's size (fx-graph-dag D3). A graph
composite needs a third rule, for a node whose inputs come from different reels at different
rasters. It would be the conform rule (F-MED-004) applied per edge.

## Pros

- **Track mattes from any track:** a logo track masks a footage track without a reel.
- **Effects that read another layer:** displacement by a map, a key from a clean plate, a light wrap
  from the background.
- **A Viewer on any node:** "show me track 2 alone with its matte" without soloing.
- **With node keys, an edit above keeps the work below** (trick 2): the largest gain is in cache
  reuse, not in expressiveness.

## Cons

- Two ways to say one thing (a reel vs a subgraph, a track stage vs a Merge with a chain), unless
  one is defined as the other (the Merge law).
- **A cross-edge breaks "what plays here" for the person:** a hidden matte track still decodes. The
  coverage and decode questions must name it, or a warm-looking moment stutters.
- **A node view of a 2000-event timeline is unreadable.** Resolve and Nuke show a graph only for one
  clip or one shot, never for an edit.

## What users say

> Any layer in your composition, no matter where it sits in the layer stack, can be used as a Track
> Matte source for any other layer.
> — Adobe After Effects User Guide, "Track mattes and traveling mattes",
> https://helpx.adobe.com/after-effects/using/track-mattes-and-traveling-mattes.html (accessed 2026-11-20)

> 2023 new track matte feature needs some work
> — thread title, Adobe After Effects Ideas,
> https://community.adobe.com/t5/after-effects-ideas/2023-new-track-matte-feature-needs-some-work/idi-p/13339012 (accessed 2026-11-20)

> every superimposed clip in a stack is automatically connected into a cascading series of Merge nodes
> — DaVinci Resolve 18.6 manual (mirror), "Creating Fusion clips",
> https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part1487.htm (accessed 2026-11-20)

> Nuke does not know that the extra area is going to be black or another constant color.
> — Nuke User Guide 8.0, "Copying a Bounding Box from One Input to Another",
> https://learn.foundry.com/nuke/8.0/content/user_guide/reformatting_elements/copying_bbox.html (accessed 2026-11-20)

## How it fits Kinotrain

CAPABILITY: openspec/specs/video-plan/

**What exists already.** We are closer than it looks:

- **Track stages and reels are already nodes.** A track stage is "a track as an implicit reel", and a
  reel instance is a sub-plan (`LayerInput::Nest`). The plan is a tree of folds. Vegas's parent/child
  is the same tree.
- **v2's Merge is the composite's own operation.** The Merge law (fx-graph-dag D5, t3204) says that
  an event's two-arm graph equals two stacked tracks.
- **There is a cross-layer input in sound already:** the compressor keyed by another track (R.65).

**What would have to change, by place:**

- **`frame_plan`** (`src/libs/kt-engine/src/video_plan.rs`, `FramePlan.layers`). Today it is a
  `Vec<PlanLayer>` folded back to front. It would become a program lowered from the graph, the same
  way `plan_chain` lowers a chain's graph in v2: `Hold`/`Load`/`Merge` registers, with the fold as
  the default lowering, so a timeline with no wires emits today's op list byte for byte (invariant 7:
  one builder, one executor).
  - A cross-edge names another track's output AT THE SAME timeline moment. An input from another
    time is a stream stage's question, not a wire.
  - Cycles are refused by order, as in v2.
- **Coverage and decode** (`layers_at`, `wanted_frames` in `src/libs/kt-engine/src/video.rs`;
  invariant 4). A moment's need becomes the set of leaves its graph REACHES, including a hidden matte
  track, not the visible stack. The bars, prefetch, the decode scale step and the starvation check
  (`FramePlan::starved`) all ask that one question. A leaf that is only a matte may need only its
  alpha or luma: a cheaper decode step is possible but is a separate decision.
- **Keys** (`plan_key` in `src/libs/kt-gpu/src/engine.rs`, `durable_hash` in
  `src/libs/kt-engine/src/render_cache.rs`, `content_hash` in `video.rs`). Today one hash covers the
  whole moment. A graph allows Merkle keys per node (trick 2), so a kept "everything below track k"
  survives an edit above it.
  - It needs a memory decision: the kept intermediates of every moment, under the memory ledger.
  - It needs a definition of "kept" for the bars. The bars stay per moment; a node's result is only
    an accelerator.
  - Determinism is untouched: a node's bytes are a pure function of its key (invariant 11).
- **Timeline scroll** (invariant 8): unchanged, because no graph is evaluated by scrolling.
- **Rest** (invariant 12): a node view must not compose at rest. A Viewer shows a kept node or
  nothing.

**What stays out:** sound. The lanes never join, and the mix is already a sum; the sidechain key is
its cross-edge.

## Estimate

| What | Answer |
|---|---|
| Size | XL: the plan's shape, the keys, coverage, the file (edges between tracks), a UI for edges |
| Where the main work is | kt-engine `video_plan.rs`/`video.rs` (lowering, need), kt-gpu (registers already exist from v2), kt-model (edges in `Track` or a composite graph), kt-app (the edge UI) |
| Project format | yes: an edge between tracks; a file without edges stays byte-equal |
| Tests that will have to be written | the fold lowering byte-equal to today's plan; the Merge law for tracks; a hidden matte track counted in coverage; node keys survive an edit above; a cycle refused |
| Depends on | F-UI-008 v2 (registers, Merge), F-CMP-003 (track blend modes), F-TL-004 (reels as subgraphs), F-MED-004 (size rule per edge) |
| Breaks compatibility | no, if the default lowering is the fold |

## Verdict

**Horizon, researched.** Of the four tricks, the one that pays without any new UI is trick 2: node
keys, so an edit above keeps the work below. It could come before any graph UI, over today's tree of
tracks, stages and reels.

Cross-edges (track mattes from any track) are the smallest visible step. Resolve's lesson says a
whole-timeline node view should not be built.

Questions for the owner:

1. **How far, if ever?**
   - (a) a tree only, as Vegas's parent/child: we have it as track stages and reels;
   - (b) a cross-edge, as After Effects: a track's matte, or an effect's second input, from any
     track;
   - (c) a node view of the frame, as the Fusion page or Nuke.
   Recommended: (b), when he asks for a matte across tracks, with node keys first if a kept
   background is wanted.
2. **Where is an edge drawn?** On the track (a "matte: track N" choice in the track window, the After
   Effects way, with "the track above" as the fast default), or in a graph window of the moment.
3. **Does a track used as a matte still show?** After Effects hides it; Vegas's mask parent is
   consumed. A rule is needed before any file carries an edge.
4. **Is memory spent on kept intermediates?** Node keys make "edit the title, the background stays
   kept" possible. It costs one kept picture per node per moment, under the memory ledger. An
   alternative is only the stage just below the edited track.

## Sources

- Adobe, "Track mattes and traveling mattes":
  https://helpx.adobe.com/after-effects/using/track-mattes-and-traveling-mattes.html (2026-11-20)
- Adobe community, "Feature Focus: Selectable track matte layers have arrived":
  https://community.adobe.com/t5/after-effects-beta-discussions/feature-focus-selectable-track-matte-layers-have-arrived/td-p/13110013
- Adobe community, "2023 new track matte feature needs some work":
  https://community.adobe.com/t5/after-effects-ideas/2023-new-track-matte-feature-needs-some-work/idi-p/13339012
- Resolve 18.6 manual (mirror), Fusion clips:
  https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part1487.htm
- Blackmagic forum, a Fusion clip black in the timeline: https://forum.blackmagicdesign.com/viewtopic.php?p=432723
- Foundry, Merge node: https://learn.foundry.com/nuke/content/reference_guide/merge_nodes/merge.html
- Foundry, copying a bbox: https://learn.foundry.com/nuke/8.0/content/user_guide/reformatting_elements/copying_bbox.html
- Our design: `openspec/changes/fx-graph-dag/design.md` D3, D5 (the Merge law)
