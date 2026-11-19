# Design: fx-graph

> Revision 2 (2026-11-18), after the owner's answers (kkt-26.11.18b lines 21–24, registry U7):
> the graph REPLACES the strip; "везде в окнах разных единым стилем в одном месте чтобы не
> скролить … графовое представление эффектов? типо как в blender? все таки у нас композинг должен
> быть и вообще говоря более сложные графы". Revision 1's two-rail row is superseded by a node
> editor; its defect fix (D1, D2) stands unchanged.
>
> Revision 3 (2026-11-18), his answers to Q1–Q5 (kkt-26.11.18b lines 26–29, registry U8): nodes laid
> out automatically by order; "я бы хотел 2-3 ползунка, но и нужно учесть как уместить огромные
> эффекты типо isp чтобы не нагромождали" — 2–3 main sliders in a node, a huge device kept compact
> (D4a); layers and the track's effects shown as read-only boxes; Slowed keeps one switch in a layer
> for now; sound for a device on a video track — a separate later change (D9). No question is open.

## Context

- Claims: kkt-26.11.18b item 3 (U3) and his answers (U7), quoted above and in `proposal.md`.
- Feature note: `docs/features/ui/editing/fx-chain-graph.md` (F-UI-008) — Blender compositor and
  geometry nodes, Resolve Fusion and Color, Nuke, Natron, Houdini networks, Ableton/Bitwig/REAPER,
  and the five core editors. What makes a node editor fit without scrolling: one key to fit, nodes
  that collapse to their header, groups that turn a sub-graph into one node, the form BESIDE the
  graph rather than inside the node.
- His standing decisions this design keeps (`docs/user-feedback.md`):
  - R.33 S2 / C2 and rule 17 — ONE widget for every chain (now: one node editor).
  - R.35 item 3 — the morpher's source swap is its purple first element (now: a purple source node).
  - R.37 item 2 — the warp's off switch on the picture is the slot's own switch; no second switch of
    one meaning (kt-app rule 9).
  - R.38 item 12 — the layer strength acts on picture AND sound at once.
  - R.63 — no new window; parameters go where the person already is. The node editor lives INSIDE
    the existing windows; the picker opens inside it.
  - R.66 — the form contract; the forms (deck face, ISP panel, passport grid, automation) are kept
    and opened beside the graph for the selected node.
  - 2026-11-03 (sim-audio-pairs) — a device recording picture and sound simulates both, one device
    state; the sound half "visible and switchable off".
  - 2026-11-07 — the program at rest costs nothing, measured on a heavy project.
  - Invariant 7 — one composition path: whatever graph the person draws, ONE plan builder turns it
    into the plan and ONE executor (the GPU) runs it.

## D1. The defect: the sound half of a device in a morpher is a ghost

**Mechanism.** `MorphPatch::chain_for(kind)` appends `derived_pairs(kind)`: for each ENABLED paired
slot on the other side, the counterpart is appended "unless this side already holds an n-th one of
that kind" — and the count of what this side holds filters out bypassed slots
(`count(own, own_off, here)`). So:

1. the half is never in `patch.audio_fx` → `merge_chains` never shows it → no checkbox;
2. a sound half added by hand and switched off counts as "not held" → the derivation puts it back;
3. symmetric: a patch carrying only `vhs_tape_sound` derives `vhs_tape` on the picture, also
   un-switchable.

**The class: a slot that the render has but the edited model does not.** Walked:

| Place | What the render gets beyond the edited slots | Found |
|---|---|---|
| Morpher patch, paired device | the counterpart half (`derived_pairs`) | **the defect** — fixed here (D2) |
| Morpher patch, time warp | `chain_for(Video)` adds the warp from `audio_fx` | the same ghost: the picture's slowing by a layer cannot be switched off alone, while on an event it can (R.37 slot checkbox). kept as one switch for now (his answer Q4) |
| Morpher patch, layer strength | `chain_for(Audio)` adds `LayerMix` from `video_fx` | by his decision R.38 item 12 ("at once"); shown as one node with a tie and ONE dot — not a defect |
| Event, paired device | — real slots on both events (`sync_pairs_to_link_group`) | clean |
| Event, warp | — real slots on both events (`sync_warp_to_link_group`) | clean |
| Track chain, paired device | nothing: a track has no partner track, VHS Tape on a video track has no sound | a gap of the same family (no pair at all); a later change `track-device-sound` (his answer Q5, D9) |
| Composition instance chain | an instance is an event: the event rule | clean |
| Preset / pasted chain into a patch | goes through `SetMorpher` → the new rule (D2) | covered |
| Bake of a morpher | `apply_to` reads `chain_for` → real slots on the pieces, then the event rule | clean after D2 |
| kt-cli `probe` | prints `chain_for` | prints the real halves after D2 |

## D2. The fix: both halves are real slots of the patch

A rule in the model, beside `sync_pairs_to_link_group`, applied inside `SetMorpher` (and therefore in
one undo step, including under `apply_coalesced`): `sync_pairs_in_patch(patch, edited_side)`.

- The n-th paired slot on the edited side ↔ the n-th counterpart slot on the other side: values and
  curves copied from the edited side, a missing one appended at the end of the other chain, an extra
  one removed. **Presence is shared, bypass is each half's own.** The edited side is known to the
  window (the lane the gesture was on); a model-only `SetMorpher` (paste, preset, kt-cli) without a
  side takes the picture side as the leader, the sound side when the picture holds no paired slot.
- `derived_pairs` is removed; `chain_for`/`auto_for` return the patch's own enabled slots (plus the
  warp and the strength, unchanged).
- `Project::normalize` materializes the halves once for patches read from older files (the only
  migration place, precedent `migrate_gen_to_events`). It derives exactly what `derived_pairs`
  derived, so an old project renders byte for byte as before (t2991).
- **The device state stays one when both are on**: sim-audio-pairs D3 — each half reads the shared
  rows and the timeline time (`AudioState::place`), not the other half's runtime state. Switching the
  sound off removes one node from the audio chain; the picture's bytes do not move (t2992), and with
  both on, a dropout is still seen and heard at the same tick (t1467 stays).
- One text for both rules: the matching of the n-th occurrence moves into one helper used by
  `sync_pairs_to_link_group` and `sync_pairs_in_patch` (kt-model CLAUDE "one rule").


## D3. What the engine executes today, and how a graph maps onto it

| What the person would draw | What the engine runs today | v1 (editor over today's model) | v2 (the engine executes a DAG) |
|---|---|---|---|
| a line of effects on an event | `fx` vector → `plan_chain` (video), `AudioNode` chain (audio) | a line of nodes from the event's Source to its Output | the same; a line is the degenerate DAG, byte for byte |
| a device on picture and sound | two slots in two chains (event pair, or patch after D2) | ONE node with a picture lane and a sound lane | the same node, lanes may be fed by different branches |
| a morpher layer over events | patch slots appended to each target's chain, strength as `LayerMix` | a purple GROUP node after the event's chain (Tab to enter) | a group node; its inside may branch |
| a composition (reel) placement | a nested layer / sub-mix (`LayerInput::Nest`, `NestFetcher`) | a GROUP node with the reel's name (double click enters the reel on the timeline) | a group node with published parameters as its sockets |
| a track stage | the track's events composited, then its chain, level, blend down | a frame "Track" with its chain; Output → Composite | the same, or a Merge node per track (v3, below) |
| a sidechain key | the key track's mix into the compressor (`track_window`) | a dashed sound wire from a "Key: <track>" node into the compressor's key socket | the same edge, any sound node as its source |
| a mask | per-event/per-slot masks (`ClipMaskPlan`) | a mask badge on the node (opens the masks form) | a mask socket fed by any picture node (a luma/alpha matte) |
| a branch / merge (blur only the highlights and add back) | **not expressible** | refused: the link-drag shows "branches arrive in v2" | `Split` implicit (one output, two wires), `Merge` node (mode, mix) |
| transitions | a pair blended into one layer | not in the graph (they live on the timeline) | not in the graph |

**v2 in the engine (a separate change, not this one):**

- Model: a chain becomes `FxGraph { nodes: Vec<Node{id, kind, values, auto, bypass, masks, pos?}>,
  edges: Vec<Edge{from:(id,port), to:(id,port)}> }`; slots get STABLE ids (today they are indices).
  A graph that is a line serializes as today's `fx` vector (no new bytes for every existing project).
- Video: `video_plan` builds per layer a DAG of passes instead of a `LayerChain`; `plan_chain` still
  fuses each LINEAR run between a split and a merge (concatenation, point-wise runs, K× barriers
  untouched); the executor gains multi-input passes — it already has two-input ones (the transition
  pair, the stream stage's second input, the mask point). One plan builder, one executor: invariant 7
  holds by construction; keys (`fx_hash`, `plan_key`, `durable_hash`) hash the edges too.
- Audio: the `AudioNode` chain becomes a DAG evaluated in topological order on the render thread;
  state keyed by (clip id, node id) — invariants 2 and 3; a split copies a buffer from the pool, a
  merge sums in a fixed order (invariant 11).
- Determinism: a line graph renders byte for byte as the chain (GOLDEN_VIDEO/AUDIO); a test
  holds every built-in chain shape of the showcase.
- **v3 (horizon, not planned):** the timeline's own composite as a graph (tracks as Merge nodes, a
  Viewer anywhere). Named so v2's model does not close the door; nothing is promised.

## D4. The node editor (v1)

**One widget** `node_editor` replaces `widgets::chain_strip` in the FX window (an event and its link
partners), the track window, the morpher window and the composition's instance chain. Same style,
same keys, same nodes everywhere.

- **Canvas.** Opens framed: every node fits the graph area (Home = frame all, as Blender). Wheel =
  zoom at the pointer, middle drag (or Space+drag) = pan. Below a readable zoom a node shows only its
  header (Blender's collapsed node); H collapses/expands the selected nodes by hand. No scrollbars:
  the graph never scrolls, it zooms. The view (zoom, pan) is per window session.
- **Layout.** v1 computes it from the order (columns by processing order, the picture lane above the
  sound lane, groups and frames as boxes); nothing is stored (his answer Q1: automatic by order).
  Dragging a node changes only its ORDER (drop it on a wire); free placement waits for v2's slot ids.
- **Sockets.** Picture — yellow circle; sound — green circle; key (sidechain) — grey diamond. A wire
  carries one kind; picture never connects to sound.
- **Nodes.**
  - Source (the event's media; for a morpher the purple source swap with its own switch; for a
    generator its kind) and Output (→ the next stage). Pinned: they cannot be deleted or moved out of
    order.
  - Effect: header (switch, name, kind glyph, a curve mark ∿ if automated, a mask mark ◐) and its
    sockets, then at most THREE main sliders (D4a). The full form is beside the graph.
  - Device (VHS Tape, Datamosh, Camera ISP): ONE node, two lanes — "picture" and "sound", each with
    its own switch; the header's switch (M) turns both. If its two halves stand at crossing positions
    in their chains (VHS before Datamosh on the picture, after it on the sound), it draws as two
    linked half-nodes with a dotted tie, so no wire runs backwards.
  - Group: a morpher layer (purple), a composition (blue), a track stage (frame "Track"). Collapsed
    to one node with its sockets and a count; Tab enters (the graph shows the inside, a breadcrumb on
    top), Tab or Esc leaves. Inside an event's graph a layer group is read-only (edit it in its own
    window: double click opens it).
  - Geometry (the event's leading node) stays first after Source, as today.
- **The form beside the graph.** The window is split: the graph (left, or top when the window is
  narrow) and the selected node's EXISTING form (right/bottom): deck face, ISP panel, passport grid,
  EQ graph, masks, automation. The split remembers its ratio. Nothing in the forms changes.
- **Picker.** Shift+A, a right click on empty canvas, or dragging a wire end into empty space opens
  the existing picker gallery as a panel inside the graph area at the pointer; the chosen effect is
  inserted where the wire was dragged from (Blender's link-drag search). Dropping a node on a wire
  inserts it there.
- **Viewer.** Ctrl+Shift+click a node: the preview shows the picture AFTER that node (the chain cut
  there, the plan's own code), with a "Viewer: <node>" badge; Ctrl+Shift+click empty canvas or Esc
  returns. A sound node: solo the event's sound up to that node while playing. (Stage v1d — it needs
  the plan to take a cut point; the cut chain is an ordinary shorter chain, invariant 7 holds.)
- **What v1 refuses.** Connecting one output to two inputs, a second input to a node, a wire across
  kinds, a cycle: the wire snaps back and the status line says why ("branches arrive in v2"). No
  half-working branch is drawn.

### D4a. Main sliders, and a huge device that stays compact (his answer Q2)

- **The passport names the main rows.** `kt-plugin-api` `UiDecl` gets `main: &'static [&'static
  str]`: at most THREE row names, each a numeric row (slider) or a choice row (a drop-down chip); a
  device names them PER LANE (`main` for the picture, `main_sound` for the sound half's passport). A
  plugin that names none gets its first ≤3 numeric `Smooth` rows; a simulated device MUST name its
  set (the registry refuses one that does not: a "first three" of ISP would be `qe_r`, `qe_g`,
  `qe_b`). Checked for every built-in plugin by t2998.
- **A node's height is bounded**: header + at most 3 slider rows per lane + one lane row per lane.
  ISP has ~80 rows, VHS Tape ~40; the node never grows with the passport. Everything else — the
  deck face, the ISP panel with its blocks, the curves — is the side form of the selected node.
- **The node's sliders ARE the common row widget** (R.66: numbers only through the common row) at
  node width; a drag is one undo step; a curve on the row shows ∿ on the slider and the slider sets
  the key at the cursor, as the form does (one rule of editing a curved row).
- **When zoomed out** past readability the sliders fold first, then the node folds to its header —
  the fit (Home) never scrolls.
- Main sets proposed (chosen in v1b from the passports, shown in the live screenshots):
  Camera ISP — picture: `look` (chip), `exposure_ev`, `analog_gain`, `sharpen_amount`… trimmed to 3
  with the look as the chip; sound: `mic_ns_strength`. VHS Tape — picture: the tape wear, tracking,
  the head clog; sound: the audio track (Hi-Fi / Normal chip). Datamosh — picture: the codec size,
  the broken-keyframe rate; sound: the audio bitrate.

ISP as a node (selected; its full panel beside it):

```
┌─ Camera ISP ──────────── ⏻ ∿ ┐  ┌ Camera ISP ─────────────────────────────┐
│ look  [ Phone night  ▾ ]      │  │ Sensor │ Optics │ ISP │ Microphone      │
●picture                 ⏻     ●  │ full well … QE … read noise … PRNU …    │
│ exposure   ▮▮▮▮▯▯▯  +0.7 EV   │  │ demosaic … NR … sharpen … tone … YUV …  │
│ gain       ▮▮▮▮▮▮▯  ×8        │  │ (the existing panel, unchanged; scrolls │
●sound                   ⏻     ●  │  inside the form only, R.66)            │
│ noise sup. ▮▮▮▯▯▯▯  60 %      │  │                                         │
└───────────────────────────────┘  └─────────────────────────────────────────┘
  ≤ 3 rows per lane, whatever the passport holds
```

### Mockups

One event's graph (FX window; a video event linked to its audio; VHS Tape selected; the form right):

```
┌ graph ─────────────────────────────────────────────────┐┌ VHS Tape ─────────────────┐
│ ┌Source┐  ┌Geometry┐  ┌──VHS Tape─────┐  ┌Glow┐  ┌Out┐ ││ [deck face]               │
│ │street├─●┤        ├─●┤● picture  [⏻] ├─●┤    ├─●┤   │ ││ Tape  Heads  Tracking     │
│ │ .mp4 │  └────────┘  │               │  └────┘  │   │ ││ Sound: Hi-Fi | Normal     │
│ │      ├─●──── EQ ───●┤● sound    [⏻] ├─●── Comp ─●┤   │ ││ ...                       │
│ └──────┘              └───────────────┘          └───┘ ││ [automation]              │
│  ●yellow = picture   ●green = sound      Home: fit all ││                           │
└────────────────────────────────────────────────────────┘└───────────────────────────┘
```

The device with its sound switched off (the sound lane dims, its wire bypasses the lane):

```
 ┌──VHS Tape─────┐
─┤● picture  [⏻] ├─
 │               │
─┤○ sound    [ ] ├─   (dimmed; the sound passes through untouched)
 └───────────────┘
```

A morpher batch (morpher window; Datamosh's sound off, Camera ISP whole):

```
┌Source swap┐  ┌──Datamosh──────┐  ┌──Camera ISP──┐  ┌Strength┐  ┌Out┐
│ (original)├─●┤● picture  [⏻]  ├─●┤● picture [⏻] ├─●┤  ⏻ 80% ├─●┤   │
│  purple   ├─●┤○ sound    [ ]  ├─●┤● mic     [⏻] ├─●┤        ├─●┤   │
└───────────┘  └────────────────┘  └──────────────┘  └────────┘  └───┘
                                   (Strength: one switch for both, R.38 item 12)
```

The same layer seen from an event under it, collapsed, then entered with Tab:

```
 Source ─● Geometry ─● Glow ─● ┌◆ Layer "mosh" (3)┐ ─● ┌▤ Track Video 1 (1)┐ ─● Out
 Source ─● EQ ──────────────●  │  picture ● sound ●│ ─● │ sound ● ← Key: Music│ ─● Out
                               └──────────────────┘    └────────────────────┘
 Tab on the layer →   Event › Layer "mosh"  (read-only here; double click opens its window)
```

The fit on a long chain (24 effects): zoomed out, every node is a header chip; Home re-fits:

```
 [Src]●[Geo]●[Crv]●[Glw]●[VHS]●[Bl]●[Shp]●[Grn]●[Mir]●[Zm]●[Pan]●[Grd]● … ●[Out]
 [Src]●[EQ]●[VHS·s]●[Cmp]●[Lim]●──────────────────────────────────────●[Out]
```

### Gestures (PATH lines)

- **PATH** double click a morpher block carrying VHS Tape → the morpher window opens with the node
  editor framed: the purple Source swap, a VHS Tape node with a picture lane and a sound lane → click
  the switch on the sound lane → the lane dims, its wire passes the node → play → the picture keeps
  the tape, the sound under the layer is clean → Ctrl+Z → the lane is lit again.
- **PATH** double click an event with linked audio → the FX window shows the graph framed, the form
  empty → drag from the Glow node's picture output into empty canvas → the picker gallery opens at
  the pointer → click VHS Tape → a VHS Tape node is inserted after Glow with both lanes wired → its
  deck face opens beside the graph.
- **PATH** drag the Glow node onto the wire between Geometry and VHS Tape → the wire highlights →
  release → Glow is inserted there, one undo step → drag a wire from Glow's output to a second
  input → the wire snaps back and the status line reads "branches arrive in v2".
- **PATH** select VHS Tape → press M → both lanes dim → press M again → both lit → press X → the
  device is removed with both halves → Ctrl+Z → it returns with its values and curves.
- **PATH** open the FX window on an event with 24 effects → every node fits as a header chip → wheel
  over VHS Tape → the view zooms there and its lanes show → press Home → all fits again.
- **PATH** in an event's graph press Tab on the collapsed layer "mosh" → the breadcrumb shows
  "Event › Layer mosh", its nodes greyed → Tab → back → double click the layer → its morpher window
  opens.
- **PATH** Ctrl+Shift+click the Geometry node → the preview shows the picture after Geometry with a
  "Viewer: Geometry" badge → Esc → the full picture returns.
- **PATH** open the FX window and the morpher window on the heavy showcase project → leave the
  mouse still over a node → nothing moves → no frame is asked.

### D4b. v1b as built (2026-11-19) — what differs from the plan above and why

- **The zoom is a level of detail, not a scale.** egui's accessibility rectangles ignore a layer
  transform, so a scaled graph would be pressed in the wrong place by the harness, the live
  scenarios and a screen reader, and scaled text blurs. The graph shows the detailed nodes when
  they fit the width, else chips wrapped into bands; nodes are child UIs clipped to the canvas, so
  a wide graph never stretches its window.
- **Ctrl+wheel switches detail ↔ chips, the plain wheel scrolls the window.** A graph that took
  the wheel stopped the form below from scrolling (t1219 caught it).
- **No new keys: the table's own commands act on the graph under the pointer** (v1c). Blender's
  M, X, Home, Shift+A and Tab would collide with the timeline's keys (marker, delete, start) and
  bypass the one key table (`controls.rs`, `ui-controls`). Instead, while the pointer is over a
  graph, Delete removes its selected node, the mute key switches it, zoom to fit is Frame all and
  zoom in/out are detail/chips — in every style, with the person's own bindings
  (`KtApp::graph_keys`, the mask keys' precedent on the preview). The picker by Shift+A and Tab's
  peek are not taken: the "+ effect" button and the double click on a box do it.
- **A node's slider row is the common number (`param_value`) and a slider**, its label painted:
  the row's label with its doors (reset, publication) stays the form's (t682 caught a second,
  weaker door). A curved row is named in the node and edited in the form.
- **The effects window's default height grows by the graph's band** (520 → 600): the form keeps
  the room it had.
- **Each window keeps its chains as before**: the morpher is one editor of two lanes; the effects
  window still shows each member event's chain as its own editor — one editor for an event and its
  linked audio, the context groups and the key wire are v1c.

## D5. Cost at rest

- The editor lays out once per key — model `revision()`, graph-area size, zoom, pan, selection,
  collapsed set, entered group — into cached galleys and one mesh for nodes and tessellated wires; a
  hover frame re-tints the hovered node only (the `EventPaint` precedent). Zoom and pan repaint only
  while the wheel or the drag moves; Home jumps (no animated fly).
- No polling: groups read the per-revision index (`tl_facts()`), not `effective_clips` per frame.
- The Viewer asks one composite per change, as the preview does; at rest it costs nothing more.
- Checked by an idle test with the editor open (`kt_time::work` rows, t3003) and live `idle` with the
  FX and morpher windows open on the heavy showcase (t3006); load measured on a pause, with hover,
  and with the windows open.

## D6. What is migrated and what stays

- Migrated: every `chain_strip` call → `node_editor`; the strip's copy-paste (`FxSlotBlock`,
  `FxChainBlock`), presets, master switch and right-click target move into the editor unchanged;
  `merge_chains`/`split_chains` become the lane mapping.
- Stays: every form (passport grid, deck face, ISP panel, EQ graph, masks, automation widget), the
  picker gallery, the preset library, the windows and the registry `WINDOWS`. No window is added.
- Removed: `widgets::chain_strip` (rule 17), `MorphPatch::derived_pairs`.

## D7. Project format and OTIO

- v1: no version change, nothing stored for the graph (layout computed, view per session). After D2
  a morpher patch with a paired kind serializes the counterpart slot in its other vector; older files
  are canonicalized by `normalize` (render byte-identical, t2991).
- v2: slot ids and edges; a project whose every graph is a line writes today's bytes; a branching one
  writes `graph` fields and the next format version BY CONTENT (as masks did). OTIO cannot express a
  branch: the export flattens nothing it cannot — the graph travels in `metadata["kt"]`, and a row
  goes into "What OTIO export loses" with v2 ("effect graphs with branches: carried only in
  `metadata["kt"]`; a foreign tool sees the clip without effects").
- v1 OTIO: the sim-audio-pairs row gains "and the halves held by a morpher patch".

## D8. Stages

- **v1a** (the defect): D1–D2 — the patch holds both halves; the morpher's sound lane is switchable.
  Ships first, alone: it closes the owner's complaint even before the editor exists (on today's
  strip the half becomes a visible slot).
- **v1b** the `node_editor` widget: canvas, fit, collapse, nodes, lanes, devices, picker by link-drag,
  insert on wire, M/X/H/Home/Tab, refusals.
- **v1c** the four windows switched to it; groups (layer, composition, track stage, key); the strip
  removed; live scenarios follow.
- **v1d** the Viewer (Ctrl+Shift+click) — the plan takes a cut point.
- **v2** (a separate change after v1 is in his hands): slot ids, edges, Merge/Split, mask and key
  sockets fed by any node, the DAG in `video_plan` and the audio node chain, format by content.
- **v3** horizon only: the timeline composite as a graph.

## His answers (kkt-26.11.18b U8) and what they fix

- **Q1 — layout:** automatic by processing order; nothing stored in v1 (D4).
- **Q2 — parameters:** 2–3 main sliders in a node, a huge device bounded, the full panel beside (D4a).
- **Q3 — context:** after the event's own nodes, read-only boxes for the morpher layers over it and
  its track's effects; Tab peeks, double click opens that layer's or track's window (D4).
- **Q4 — Slowed in a layer:** one switch for picture and sound stays (D1's walk records it).
- **Q5 — a device on a video track:** gets its sound in a separate later change (D9).

## D9. Follow-up, not in this change: a device's sound on a video track

His answer Q5 (a): VHS Tape / Datamosh / Camera ISP on a whole VIDEO track should also sound. A track
has no linked audio track today; the follow-up change (`track-device-sound`) gives the track's device
node a sound lane wired to the audio track(s) the person picks in its form, the device state shared as
on events. Until then the node on a track shows no sound lane and its form says the device is
picture-only there.

## Risks

- t244/t218 pin the strip; they are rewritten to the node editor by his answer "вместо полоски"
  (U7 line 21), with the same claims: the source swap first and unmovable, effects of both kinds in
  one editor, the source field takes a file.
- The live scenarios that click strip crumbs (`vhs_sound`, `stream_sound`, `camera_mic`, the morpher
  ones) move to the editor's probes.
- A node editor promises routing the engine lacks until v2: every refused gesture must say so in the
  status line, never fail silently.
