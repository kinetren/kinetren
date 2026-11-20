# Design: fx-graph-dag (fx-graph v2: branches and merges the engine executes)

> Revision 2 (2026-11-20). Base: orch `5e7a8ea9`. The owner answered (kkt-26.11.20 A5, A6; owner-checks
> Q3–Q5, numbered Q1–Q3 in this design): picture branches first (Q1 a), no inputs from other layers in
> v2 (Q2 a), Slowed, Datamosh and the pitch ALLOWED inside a branch at the cost of a second read (Q3 b).
> Revision 1 recommended Q3 a (a trunk rule); D2 "Reads" replaces it.

## Context

`fx-graph` v1 replaced the chain strip with a node editor over today's model: a chain is a vector,
the editor draws it as a line per lane, and every gesture that would make a non-line is refused with
"branches arrive in v2". The v1 design's D3 table named what v2 must add: slot inputs, a Merge node,
mask and key sockets fed by any node, a DAG in `video_plan` and in the audio node chain, the format
by content. This change designs exactly that, against the engine as it is on `3488fbe1`.

What the engine already has that v2 grows from (read on `3488fbe1`):

- **The picture chain runs as a straight program with held frames.** `kt_fx::plan::plan_chain`
  turns a layer's chain into `PlanOp`s (geometry `Run`, `Barrier`, `PointRun`, `Snapshot`, `Mix`),
  cut at "stops" so that the plan of a slice equals the piece of the plan of the whole (t306).
  kt-gpu's chain loop (`engine.rs`, `FxOp::Snapshot`/`Mix`/`MaskSnapshot`/`MaskMix`) holds up to
  TWO frames — the layer strength's dry side (`snap`) and a slot mask's dry side (`msnap`) — and mixes
  the current frame with one of them, fitting sizes by `fit_into`. That is a branch of length zero
  and a merge, twice, already.
- **The layer composite has nine blend modes** (`BlendMode`, R.62 B): `Normal` is the old
  `alpha_over` byte for byte, the others are the shader's `blend_over`. A merge needs exactly these.
- **The sound chain** (`ClipProcessor`) runs `AudioNode`s in slot order on one buffer; the layer
  strength takes a dry snapshot and blends it back after a `DryRing` that aligns it with the
  look-ahead the nodes between "ate" (PDC, R.48). The compressor's key input is a second stream
  (the sidechain key branch, R.65). The warp is the chain's input, not a node (`audio-fx`: "Warp is
  not a node; it consumes the source exclusively and cleanly").

## Goals / Non-Goals

**Goals**

- A chain can branch and merge, in the picture and in the sound, and the preview, the export, the
  reel strip, the poster, the picker cells and the Viewer all show it through the one plan.
- Every project whose chains are lines renders, saves and hashes exactly as before.
- The editor makes branches with Blender's gestures; the layout stays automatic (his U8 Q1).

**Non-Goals (v2)**

- Inputs from outside the layer: another track's picture, a generator node as a second source (Q2 a,
  his answer "Не в v2").
- The timeline composite itself as a graph (v3 horizon, D5).
- Multi-input plugin kernels: the plugin contract stays single-input; Merge is core.
- Picture↔sound wires (sound driving a picture parameter is modulation, not routing).
- Free node placement (his U8 answer: automatic; a limit he set stays).

## D1. The model: a line with its exceptions written down

A chain stays `fx: Vec<FxInstance>` with `fx_bypass`, `fx_auto`, slot masks — everything indexed by
slot, as today. It gains ONE parallel, sparse field:

```text
fx_in: Vec<SlotInputs>         // empty = every slot takes the slot before (today's line)
SlotInputs { main: FxIn, second: Option<FxIn>, matte: Option<(FxIn, MatteFrom)> }
FxIn = Source | Slot(usize)    // Source = the chain's input (the decoded frame, the patch's target)
MatteFrom = Alpha | Luma | InvAlpha | InvLuma
```

- **Order is processing order.** An input names only an EARLIER slot (or the Source), so the slot
  vector is always a topological order and a cycle cannot be written. The editor's wire that would
  point forward re-sorts the slots (stable topological sort, one command); one that would close a
  loop is refused.
- **The Output is the last slot** (as today). A slot that reaches the Output through no path is
  dangling: kept, saved, drawn dim, skipped by the plan like a bypassed slot.
- **Why not node ids and an edge list** (v1 design D3 sketched `FxGraph{nodes, edges}`): every
  machine in the program addresses a slot by index — bypass, curves (`fx_auto`), slot masks,
  published parameters (`ParamOwner`), the pair rule's n-th occurrence, the morpher splice, the
  lead's `geometry_pos`, copy/paste blocks, `FxStateStore`. Ids would rewrite all of them for no
  gain: with inputs restricted to earlier slots the index IS a stable topological id inside one
  chain revision, and a line needs no field at all. Rejected.
- **Merge** is `FxInstance::Merge { mode: MergeMode, mix: f64 }` — a core kind (the plugin
  registry does not list it; the picker shows it as its own first row, as it shows the morpher's
  source swap). Picture `MergeMode` = the nine `BlendMode`s; sound = `Sum` (Background + mix ×
  Foreground) and `Crossfade` (Background × (1 − mix) + Foreground × mix). `mix` takes a curve like
  any parameter. Its inputs: `main` = Background, `second` = Foreground, `matte` optional.
- **Bypass** of a slot passes its `main` input; of a Merge, its Background. `fx_off` turns the whole
  graph off, as today.
- **Validation** (`Project::validate`, the same place that checks chains today): inputs earlier; a
  picture slot's inputs are picture slots and a sound slot's sound slots (the lanes never join); a
  `second` or `matte` only on a Merge (and a compressor's key, D4); the leading Geometry node has no
  branch before it; at most `MAX_HELD = 4` frames held at once by the lowered program (D3) and at most
  `MAX_READS = 4` reads per layer (D2).
  A file breaking any of these is refused with the slot's address, as a broken chain is today.
- **`Debug` of `ClipEvent` gains the field.** `content_hash`, the per-event memo key and the kept
  mix's `AudioClip.fp` are made from it, so every revision hash moves ONCE with the build that adds
  it — the precedent of `overrides` (R.59): the reel previews' disk rows are recomputed once on
  opening; the render cache on disk is salted by the build anyway; frames and samples do not move.
  Hand-writing `ClipEvent`'s `Debug` to hide an empty field was rejected: a second text of the event
  that every new field must remember.

## D2. What the graph can express

| What a person draws | v2 | How it runs |
|---|---|---|
| a line of effects | yes (today) | unchanged, byte for byte |
| a branch: one output feeding two nodes | yes | the output is held, each arm runs from it |
| a merge of two arms (mode, mix) | yes | Merge: Background ← one arm, Foreground ← the other |
| an effect through a matte made from the picture | yes | Merge's Matte ← an arm (luma key, alpha); per-pixel mix |
| blur only the highlights, laid back | yes | Source → Luma key → Blur → Merge(Screen) over Source |
| parallel compression / a reverb send | yes (sound) | Source → Compressor; Merge(Sum) with Source |
| a compressor keyed by a filtered copy of its input | yes (sound) | the compressor's key socket ← an arm of the same chain |
| a morpher layer's patch that branches | yes | the patch graph spliced after the target's Output (D6) |
| a device (VHS / Datamosh / ISP) on both lanes | yes | each half wired in its own lane; the pair rule mirrors presence and values, NOT wires |
| Slowed / Datamosh / the pitch's edge reading inside an arm | **yes** (Q3 b) | the arm opens a second READ of the layer's source (below) |
| the leading Geometry node inside an arm | **no** | it places the frame and sets the decode step; an arm's own placement is a Transform/Geometry slot |
| another track's picture as an input or matte | **no** (Q2 a) | refused; the track window's masks and the composite stay |
| a generator node as a second source | **no** (Q2 a) | refused |
| a loop (feedback) | never | impossible by D1's order; a frame's past is a stream stage's job |
| a wire from picture to sound | never | the lanes never join (v1 rule) |
| a dangling node | kept | drawn dim, not run, saved |

**The trunk** is the chain's input up to its first branch point. **A time kind** is a slot that
decides how the layer READS its source rather than what it does to a frame: the warp (Slowed's rate,
`warp_rate`), the stream stage (Datamosh's codec path, whose `front` is the picture slots before it)
and the pitch's edge reading (`reads_across_edges`, the context gate). On the trunk a time kind is
what it is today. Inside an arm (his answer Q3 b, "Разрешить в ветке") it opens a **read**.

**Reads.** A layer has read 0, the trunk's: one source at one material moment, as today. A time kind
`T` in an arm opens read `k`: a derived clip of the same event whose chain is the ancestors of `T`
with `T` as its Output. That derived chain reads EXACTLY as an event whose chain it were would read:
its own trunk time kinds decide its material moment (`clip_warp_of` over its slots: today's rule, the
first warp slot counts), its stream stage keeps its own history, its edge reading its own window. Its
own arm time kinds open nested reads by the same rule. The layer's lowered program takes read `k`'s
finished frame in place of `T`'s output (`Input(k)`, D3); the ancestors that only `T` needs run in
read `k`, the shared ones run in read 0 as well. So a read is not a new reading machine: it is one
more clip shaped as today's, and every engine rule about reading applies to it unchanged:

- `layers_at` / `wanted_frames` list each read's frames (a read with Datamosh: its stream frames;
  with frame blending: both sides), so the prefetch warms both and the composite reads exactly what
  was warmed (invariant 4);
- the decode scale step is per read, from that read's leading geometry series (a read shares the
  trunk's lead, so in practice the same step);
- the stream history is keyed by (event, read): Datamosh in an arm keeps a SECOND history, counted
  by the stream memo budget and the memory ledger as its own;
- the coverage strip of the event draws one row per read, and the event is warm only where every
  read is (the bars "show two reads", the cost he accepted); the performance panel and `kt-cli
  probe` name the reads and their decode cost;
- the sound: an arm's warp reads its own source window (`chain_source_window_ext` per read), its own
  `fill_source`; the kept final mix's scene hash follows the event's `Debug`, which carries `fx_in`.

**The read law.** An event whose graph merges arm B (holding a time kind) over arm A renders the
picture of two stacked copies of the event, the upper with B's path as a LINE (Slowed and all) and
the lower with A — the Merge law (D5) extended to time: one law, so a read cannot invent a new
meaning for speed or history. t3226.

**The cost, stated.** Every read is a decode, a cache footprint and a run of its exclusive ancestors:
two reads cost the layer twice the reading. `MAX_READS = 4` per layer bounds it; a fifth is refused
by the editor with the reason, and by the reader with the slot's address. Without an arm time kind a
graph has one read and asks exactly what its line asks (invariants 4, 5, 8; t3205).

The leading Geometry node stays on the trunk: it places the frame on the canvas and sets the decode
step for the layer; an arm that needs its own placement takes a Geometry/Transform slot of its own.

**The pair's speed is the trunk's** (applied in v2a). A link pair consumes its source at one speed
(R.37); with reads, that speed is the trunk's warp. An arm's warp is the arm's own reading, so the
mirror treats it as no warp: moving Slowed into an arm of the picture takes it off the linked sound
in the same step, moving it back puts it on again. A chain still holds ONE warp slot (D11.5's
`TwoTimeWarps`), so an arm with its own speed means a trunk at 1×.

## D3. The picture: `plan_chain` lowers a graph into a straight program

One builder, one executor (invariant 7). The graph is lowered where the chain is planned today.

**Lowering** (kt-fx, `plan_chain_in` and its callers): walk the slots in order with a "current" frame.

- Before slot `i` whose `main` is not the current frame: `Load(r)` the held frame it names (the
  Source is held as `r0` at the start when anything names it).
- After slot `i` whose output is named by a later slot other than the next, or by a `second`/`matte`:
  `Hold(r)`; the register is freed after its last reader (a liveness count, fixed by the slot order,
  so the program is a pure function of the chain).
- A Merge: `Merge { bg: r, matte: Option<(r, MatteFrom)>, mode, mix_k }` — the current frame is the
  Foreground when the Foreground arm ran last; otherwise the lowering loads it; the rule is one and
  fixed so the passes are one sequence.
- Every `Hold`/`Load`/`Merge` position is a STOP for segmentation, exactly as the strength and mask
  stops are: geometry series, point-wise runs and K× barriers fuse within the linear runs between
  them, and the plan of a run equals the piece of the plan of a line made of it (the t306 law,
  extended: t3203). A stop zeroes the K accumulators; merges run at 1× (as the strength mix does).
- A line lowers to NO new op: the op list, its keys and its bytes are today's (t3200).

**Executing** (kt-gpu chain loop): the two held slots become a set of `MAX_HELD + 2` (the strength's
and the mask's keep their own, so a morpher snapshot and a mask snapshot still never share a slot).
`Hold` keeps the current texture out of the pool; `Load` makes a held one current (a later write
never touches it: every pass writes a fresh pool texture today). `Merge`:

1. fit the Foreground to the Background's size by the existing `Resample` (`fit_into`) if they
   differ — **a merge's size is its Background's**, as Nuke's B;
2. the existing blend pass: `Normal` = `alpha_over` byte for byte, the others `blend_over` — the
   same kernels the layer composite runs;
3. if `mix < 1` or a matte is wired: the existing `LayerMix` pass, dry = Background, wet = the
   blend, coverage = mix × the matte; the matte is made on the device by one new small pass
   (`matte_from`: alpha / luma Rec.709 integer weights / inverted) into a coverage texture — the
   CPU-rasterised coverage of shape masks keeps its upload path.

The alpha flags travel per side as they do for `Mix` (vhs-tape-simulation's t1052 lesson): each
held frame keeps its own fold.

**What around the plan follows from the reads, and what changes:**

- the decode scale step: `ss_taps(head_zoom)` is taken from the leading geometry series only when it
  stands before the first branch; a chain that branches first gets step 1 (a held S× frame would
  otherwise reach a merge at the wrong size);
- the stream stage's `front` = the picture slots before it in its read — on the trunk today's slots,
  in an arm the derived chain's (D2);
- `Input(k)`: a read's finished frame enters the layer's program as a held frame does; it counts
  against `MAX_HELD` while live;
- slot masks inside an arm: the mask's dry side is the slot's own input (the frame current just
  before the slot), so `MaskOp::Mix { from }` takes the lowered position, not `i`;
- the layer strength (`LayerMix`) stays a spliced marker on the trunk after the target's Output (D6);
- the Viewer (v1d) cuts to the node's ANCESTORS: the slots the chosen node reaches back to, the node
  as the Output; a line's ancestors are its prefix, so v1d's frames do not change (t2997 holds);
- the picker cells (`fx_thumbs`) insert a candidate after the selected node on its arm, through the
  same lowering;
- keys: `fx_chain_hash`, kt-gpu `plan_hash`/`plan_key`, `durable_hash` (render cache on disk) and
  the post-FX key hash `fx_in` only when it is non-empty — a line's keys are today's; a new field in
  `plan_hash` is hashed by `durable_hash` too (kt-engine CLAUDE rule);
- cost: `Merge` and `matte_from` get rows in the pass-cost table (a pass never measured is
  predicted), `Hold` adds a held texture to the frame's peak, which the memory ledger counts per
  layer as frames held;
- prewarm: `prewarm::effect_ids` reports `merge` and the matte pass when a project reaches one.

**Determinism.** Integer kernels as today, a fixed pass order from the slot order, no new
float-libm (invariant 11). On one device byte for byte; across devices within the tolerance class
of the passes a merge reuses (EXACT for `Normal`/integer modes, as the composite's).

## D4. The sound: the processor runs a graph

`ClipProcessor` keeps its nodes in slot order. A graph adds per-slot input routing:

- buffers for held outputs come from `FxStateStore::pool()` and go back in the same call (a warm
  render allocates nothing, t2684 stays);
- **look-ahead (PDC)**: each path's latency is the sum of its nodes' look-ahead; before a Merge the
  lower-latency input is delayed by the difference through a ring — the `DryRing` of the layer
  strength, generalised; the chain's total look-ahead is the Output path's. A line computes exactly
  today's `ahead` (byte for byte);
- **the sum order is fixed**: Background, then Foreground × gain, frame by frame in one pass
  (invariant 11, audio byte for byte always);
- **the compressor's key socket** may take an arm of its own chain (a filtered copy of its input).
  The key from another TRACK stays the track window's (R.65), unchanged;
- **tails**: a chain's tail is the longest path's; a dangling node does not ring;
- **state** (invariants 2, 3): nodes stay keyed by (clip id, slot). A rewire is a structural change
  as a reorder is today (`chain_fingerprint` hashes `fx_in` when present); a value edit on a graph is
  `set_params` as on a line;
- the warp, the stream's sound (`datamosh_sound` reads the picture's schedule) and the pitch's edge
  reading in an arm open a sound read (D2): the processor gets a source window and a `fill_source`
  per read, the arm's warp consumes its own source exclusively and cleanly as the chain's input does
  today; the stream's sound follows its read's schedule; the kept final mix's scene hash
  (`AudioClip.fp` from the model event's `Debug`, which now carries `fx_in`) needs no new rule;
  t3229.

## D5. How it relates to compositing

Three levels, and v2 adds only to the first:

1. **A layer's graph** — what ONE event (or a track stage, or a reel instance) is before it is
   composited. v2 is entirely here: a graph never reaches another layer.
2. **The timeline composite** — layers in the Vegas order, levels, fades, transitions (`Dissolve`
   pairs), blend modes, track stages (a track as an implicit reel), reels. It stays the plan's
   `PlanLayer` list, executed as today. Not a graph in v2.
3. **v3 (horizon, not planned)** — the composite as a graph: tracks as Merge nodes, inputs across
   layers (track mattes), a Viewer anywhere. v2's model does not close that door: a Merge is the
   same operation the composite runs.

**The Merge law.** A Merge is the layer composite's operation, so: an event whose graph is "Source
→ arm A → Background; Source → arm B → Foreground; Merge(mode M, mix 1)" renders the same picture
as two copies of that event stacked on adjacent tracks, the lower with chain A, the upper with chain
B and blend mode M, both at full level (t3204, the analogue of the compositions' law E). The law is
what keeps two composition codes from appearing: one set of kernels, two places that call it.

Where the two levels meet:

- **Track stages**: a track's chain is a graph too (same field); it runs over the composite of the
  track's events, as today.
- **Reels**: an instance's chain is the event's graph, applied to the reel's sub-composite; the
  reel's own content composes by level 2. Published parameters keep addressing slots by index (D1).
- **Transitions**: untouched. A `Dissolve` pair blends two layers' finished outputs (each with its
  own graph).
- **Shape masks** (per picture, per slot, per track) stay; a Merge's Matte is their per-pixel,
  picture-made sibling (a luma or alpha key), not a replacement.

## D6. The morpher, pairs, copy/paste

- **Splice**: today a patch's slots are appended to each target's chain (`chain_for`) and the
  strength is a `LayerMix` slot. With a graph, the patch's `Source` is the target's Output: patch
  inputs are offset by the target's length when spliced; the `LayerMix` dry side is the target's
  Output, as today. A target that itself branches is unaffected: its Output is one slot.
- **Pairs** (`sync_pairs_to_link_group`, `sync_pairs_in_patch`): presence, values and curves stay
  mirrored by the n-th occurrence; wiring is per lane and NOT mirrored (a device's picture half may
  sit in an arm while its sound half sits on the trunk — the device state is shared through the
  timeline time and rows, sim-audio-pairs D3, not through a wire). Removing a half that has a
  reader re-wires the reader to the half's own input — one rule for every removal of a slot (the
  cross, Delete, the pair mirror, a morpher bake), kept in one model helper so the graph stays valid.
- **Copy/paste and presets** (`FxSlotBlock`, `FxChainBlock`, `append_chain_block`): a block carries
  its `fx_in` relative to the block; pasting re-bases it; an input outside the block becomes the
  block's input ("input to input", the rule `append_chain_block` already states).
- **Undo**: every gesture is one command (`SetFx`/`SetTrackFx`/`SetMorpher` with the new field).

## D7. The project file and the OTIO door

- **`.kt`**: a slot whose inputs are not the line writes `"in": {"main": …, "second": …, "matte":
  [… , "luma"]}` next to its kind; a Merge writes `"kind": "merge"`. The version is set BY CONTENT:
  a project with any `"in"` or Merge is the next free format number at merge time (8 on
  `5e7a8ea9`, after the text sources' 7); every other project keeps its version and its text (t3210). An
  older build refuses a format-8 file naming the version, instead of opening it as lines and
  running the arms in sequence.
- **OTIO export** (`kt-otio convert.rs`, "the FX chain into standard `Effect.1`"): the chain stays
  an ordered `Effect.1` list with `effect_name: "kt:<kind>"` (slot order is processing order, so the
  list is a valid order); each effect's `metadata["kt"]` gains `in` when not the line; Merge is
  `kt:merge`. The envelope version is v18 by content. Import reads it back (t3211).
- **What OTIO export loses** — a row added with v2a in `docs/ktprompts/v26.10-index.md`:
  "fx-graph-dag | effect graphs with branches and Merge nodes (inputs per slot, the Merge's
  Background/Foreground/Matte) | the effects stay an ordered `Effect.1` list; the wiring is carried
  only in each effect's `metadata["kt"]` (v18 by content); a foreign editor that applied the list
  would run the arms one after another". A foreign editor never ran our `kt:` effects (the existing
  rows); what is new is that the list's ORDER alone no longer describes the picture. OTIO has no
  effect-graph schema (`Effect` is a flat list on an `Item`), so there is no standard to map to.

## D8. The editor (v2d)

His U8 answer stands: the layout is automatic by processing order, nothing stored. Within a lane
the trunk runs on the lane's line; each arm is a parallel row below it, wires routed in the gaps
(the v1 band wrap); a Merge sits where its last input arrives.

```text
 picture  In ─ Lead ─┬─ Luma key ─ Blur ───────┐
                     │                         ▼ Fg
                     └───────────────────── Merge(Screen) ─ Out
                                              Bg
```

- **Make a branch**: drag a wire from a node's output socket into empty canvas → the picker opens
  there → the picked effect starts a new arm from that output (v1 refused this).
- **Merge**: the picker's first row "Merge" (as the morpher's source swap is first in its picker);
  dropping a wire from a node onto a Merge's Foreground or Matte socket connects it; dragging a wire
  off a socket disconnects (the node then dangles, dim).
- **Insert on a wire** (v1) works on any wire, trunk or arm.
- **A time kind in an arm** is accepted (Q3 b) and the arm's node row says it is a second read
  ("reads the source again"), so the cost he accepted is visible where it is made.
- **Refusals** (rule 16, as errors in the status line): the leading Geometry into an arm, a fifth
  held branch, a fifth read, a branch on the sound lane until v2c, a wire across lanes, a wire that
  would close a loop.
- **Viewer** (Ctrl+Shift+click, v1d) on any node of any arm.
- **Cost at rest**: as v1 (D5 of fx-graph): laid out once per revision/size/zoom/selection.

## D9. The class walk: everything a chain touches

The change's mechanism: "a chain is a line" is assumed wherever a chain is read. Every reader was
walked on `3488fbe1` and again on `5e7a8ea9` for the reads; each is either unchanged, or named in D2–D8:

| Where | Reads a chain as | v2 |
|---|---|---|
| `plan_chain` (kt-fx) | runs and stops | lowers the graph; stops at Hold/Load/Merge (D3) |
| kt-gpu chain loop | ops with 2 held frames | a held set; Merge, matte pass (D3) |
| `layer_src`, `LayerChain`, curve memo | the vector | unchanged (curves resolved per slot) |
| decode scale step, `head_zoom` | the leading geo series | per read; the lead only before the first branch (D2, D3) |
| stream stage `front`, `after_chain`, stream history | slots before it | per read, history keyed (event, read) (D2) |
| frame blending, warp map, `clip_material_pos` | the warp slot | per read, today's rule over the read's chain (D2) |
| prefetch / `layers_at` / coverage strips | one source per layer | every read listed; a strip row per read (D2) |
| stream memo budget, memory ledger | per event | per read (D2) |
| masks (`ClipMaskPlan`, `MaskOp`) | slot positions | `from` = the slot's lowered input (D3) |
| morpher splice, `LayerMix` | append + strength marker | offset inputs (D6) |
| pair rules | n-th occurrence | presence mirrored, wires not (D6) |
| reels, published parameters | slot index | unchanged (D1) |
| track stages | the track's vector | a graph, same lowering |
| transitions | finished layers | unchanged (D5) |
| Viewer (v1d) | a prefix | ancestors (D3) |
| `fx_thumbs` picker cells | a candidate inserted | after the selected node, on its arm (D3) |
| keys: `fx_chain_hash`, `plan_hash`, `durable_hash`, post-FX | the vector | + `fx_in` when present (D3) |
| render cache on disk, readback memo, kept composites | the keys | follow the keys |
| prewarm, pass cost, memory ledger | kinds, passes | Merge, matte rows; held frames counted (D3) |
| `ClipProcessor`, PDC, `DryRing`, tails | nodes in order | graph routing, aligned merges (D4) |
| `FxStateStore`, `chain_fingerprint` | kinds | + `fx_in` (D4) |
| kept final mix (`AudioClip.fp`) | model Debug | carries `fx_in` (D1) |
| sidechain key (track) | the key track | unchanged; in-chain key added (D4) |
| copy/paste, presets, `append_chain_block` | blocks | relative inputs (D6) |
| `.kt`, `normalize`, validation | the vector | `in`, Merge, format by content (D1, D7) |
| kt-otio | `Effect.1` list | `in` in metadata, `kt:merge`, v18 (D7) |
| legacy import (`legacy_import.rs`) | old forms | nothing: no old file has a graph |
| `kt-cli probe` | effective events | prints the graph's arms per event |
| telemetry journal | graph edits | the wire gestures are recorded like other edits |

## D10. Stages

- **v2a — model and format**: `fx_in`, Merge, validation (`MAX_HELD`, `MAX_READS`, the lead), the
  read split as a model function (`fx_reads`: the derived chains of a graph), the morpher splice,
  pairs, copy/paste, presets, `.kt` format by content, OTIO `in`/`kt:merge`/v18, the OTIO-loss row.
  The engine still runs lines: until v2b the player and the export run a branched chain's Output
  path as a line and the editor makes no branch, so v2a is merged only together with v2b.
- **v2b — the picture**: lowering, the held set, Merge and matte passes, reads in the engine
  (`layers_at`, stream history per read, strip rows), keys, decode step rule, masks in arms, Viewer
  by ancestors, picker cells, cost rows, prewarm. Tests t3200–t3209, t3226–t3228. A branch on the
  sound lane is refused by the editor with "sound branches come next" (Q1 a).
- **v2c — the sound**: the processor's graph, PDC alignment, in-chain key, tails, fingerprint, sound
  reads. Tests t3212–t3217, t3229. After v2b (Q1 a, "Сначала картинка").
- **v2d — the editor**: branch gestures, Merge in the picker, the arms' layout, refusals; live
  scenario `node_graph_branch` with a screenshot per step; the idle check on the heavy showcase with
  a branched chain. Tests t3218–t3225.

## Risks

- **Bytes of existing projects**: guarded by t3200 (a line lowers to today's ops; GOLDEN_VIDEO,
  GOLDEN_AUDIO) and t3210 (a line project's file text is unchanged).
- **Memory**: a held frame per live branch per layer; bounded by `MAX_HELD` and counted by the
  ledger. On the owner's 4K render a layer with four held frames costs 4 × 33 MB more at its peak.
- **A read doubles the layer's reading** (Q3 b, accepted by the owner): bounded by `MAX_READS`, shown
  on the strip and in the performance panel, so a slow branched layer names its own cause.
- **The plugin contract stays single-input**: a plugin that wants two inputs (a displacement map) is
  a later change.

## The owner's answers (kkt-26.11.20, rows A5, A6)

- **Q1 — the order of delivery** (owner-checks Q3): "Сначала картинка (Recommended)" — (a): v2a+v2b,
  then v2c; a sound branch is refused with its reason until v2c.
- **Q2 — inputs from outside the layer** (owner-checks Q4): "Не в v2 (Recommended)" — (a): a graph
  works inside one layer; a matte comes from the layer's own picture.
- **Q3 — speed, Datamosh and the pitch inside a branch** (owner-checks Q5): "Разрешить в ветке" — (b):
  an arm with a time kind is a second read (D2), the layer's decode and cache cost doubles, the
  coverage strip shows two reads, Datamosh in an arm keeps a second history.
