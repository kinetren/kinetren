# Tasks: fx-graph

> His answers are in (U7, U8): the editor replaces the strip; one Blender-style node editor in every
> window, no scrolling; layout automatic by order; 2–3 main sliders in a bounded node; layers and the
> track as read-only boxes; Slowed one switch; a track device's sound is a later change (D9). The
> orchestrator gave the go for v1a. Tests reserved: t2990–t3009.

## 0. Notes first

- [x] 0.1 F-UI-008 `docs/features/ui/editing/fx-chain-graph.md` (revision 2: Blender, Fusion, Nuke,
      Natron, Houdini); editor keys `bitwig-6`, `reaper-7`, `houdini-21`, `natron-2.5`;
      `features-index.py --check` green
- [x] 0.2 proposal, design (engine mapping, v1/v2 stages, mockups, gestures, class walk, questions),
      spec deltas (`morpher`, `audio-fx`, `ui-reach`); `openspec validate fx-graph --strict` green
- [x] 0.3 registry `kkt-26.11.18b-index.md` row U3 → partial, `fx-graph`
- [x] 0.4 his answers (U7, U8) in `docs/user-feedback.md` (orchestrator) and in this design (rev 3)

## 1. v1a — the patch holds both halves (the defect)

- [x] 1.1 kt-model: one n-th-occurrence matcher shared by `sync_pairs_to_link_group` and a new
      `sync_pairs_in_patch`, called from `SetMorpher` (edited side from the command; leader rule for
      model-only callers)
- [x] 1.2 remove `MorphPatch::derived_pairs`; `chain_for`/`auto_for` return own enabled slots (warp
      and strength unchanged)
- [x] 1.3 `Project::normalize`: materialize the halves of older patches (the only migration place); a
      half held switched off counts as held (else a save turns it back on)
- [x] 1.4 tests t2990, t2991 (old file: the derived chains, idempotent), t2992 (picture untouched by the sound
      half's switch; both on equals an event-carried device); rewrite t1461b per its new text
- [x] 1.5 kt-app on today's strip: the sound half is a visible slot with its checkbox, marked with the
      speaker in a strip of both kinds (the halves share one name); t2993; live `morph_pair`
- [x] 1.6 OTIO-loss row of sim-audio-pairs extended; kt-model CLAUDE.md paragraph; What's new line;
      commit; `target-prune.py`

## 2. v1b — the node editor widget

- [x] 2.1 `node_editor.rs`: the fit by level of detail (detailed nodes, else chips in bands),
      Ctrl+wheel detail ↔ chips, middle-drag pan, Frame all / double click; layout by order, column
      width by content, sockets by kind, In/Out and the head pinned; children clipped to the canvas
- [x] 2.2 nodes: effect, device (two halves in one column and one frame, each its switch; crossing →
      a dotted tie). Groups (layer, composition, track stage, Tab, breadcrumb) and the key wire → 3.4
- [x] 2.3 gestures: drop a node on another reorders within its lane, across lanes refused with a
      reason; the picker, presets, copy/paste, right-click target as the strip had them; a cross on
      every node. Link-drag picker, Shift+A, M/X/Home/H/Tab → v1c with the key table (`ui-controls`)
- [x] 2.4 cost at rest: no animation and no repaint request; the layout is a few rectangles a frame
      while the window draws (a cache keyed by revision was not needed: t3003, live `idle`)
- [x] 2.5a kt-plugin-api `UiDecl.main` (≤ 3 rows, each half its own; a paired video kind must name
      them) and `main_rows`; the sets of ISP, VHS Tape, Datamosh and their halves; the node's sliders
      through `param_value` + a slider; bounded node height; chips carry no sliders; t2998, t2999
- [x] 2.5 i18n ×5 (`[fx.graph]`), font subsets regenerated
- [x] 2.6 tests t2994, t2995, t3001, t3003; t749/t750/t1219 scroll the window to a field the graph
      put under the harness fold; the FX window's default height carries the graph's band (520 → 600);
      live `node_graph`; commit; prune

## 3. v1c — the windows

- [ ] 3.1 FX window: the event and its partners in one editor, the form beside it (split remembered)
- [ ] 3.2 morpher window: the purple Source-swap node, both vectors as lanes (t244/t218 rewritten)
- [x] 3.3 track window: the key box with a dashed wire into its compressor (`ChainEdit.keys`); a
      composition instance's chain already uses the editor (the head is its parameters)
- [x] 3.4 context boxes: the layers over the event bottom up (`Project::layers_over`, the render's
      `windows_on`) and its track's stage with its key, read-only, double click opens their window;
      kept per model revision; t2996, live `node_graph`. Tab peek waits for the key table
- [x] 3.5a `widgets::chain_strip` removed — the three windows call `node_editor::chain_graph` (v1b)
- [ ] 3.5 the remaining the live scenarios that clicked crumbs follow the editor
      (`vhs_sound`, `stream_sound`, `camera_mic`, morpher ones); new live `morph_pair`
- [ ] 3.6 cost at rest: t3003, live `idle` with both windows open on the heavy showcase (t3006); load
      on a pause, hover and with the windows open — numbers in the report
- [ ] 3.7 specs merged, What's new, commit; prune; the orchestrator runs `finish.sh` on the stage

- [x] 3.8 found on the way: a single-lane SOUND chain (an audio event, an audio track) laid its nodes
      in the slot the layout never read — v1b drew no nodes there; wrapping wires between chip bands
      now run in the gaps

- [x] 3.9 the key table's Delete, mute and zoom commands act on the graph under the pointer
      (`KtApp::graph_keys`, `node_editor::GraphKey`); no new key; t3002

- [x] 3.10 one editor for an event and its linked audio (design D10): the pair's virtual chain with
      its slots' lanes given (`ChainEdit.lane_of`: a lane is the slot's EVENT, a video's slowed is an
      audio kind on the picture lane), a master switch at each lane's In (`master2`), context boxes
      per lane, a right click on a sound node addresses the audio event's slot; split back per event,
      a side whose vectors did not change sends nothing (else the model re-pairs a removed device);
      the toolbar's presets and copy/paste act on the pair (a slot appended goes to its kind's lane).
      The detailed graph opens only within 40 % of the window's height. t3004; t1468 brings its face
      into view; live `node_graph` addresses the pair's boxes

## 4. v1d — the Viewer (design D11)

- [x] 4.1 the Viewer: the PLAYER's project with one event's chain cut after the node
      (`node_editor::Viewer::cut`, in `push_project_to_player`) — the preview's own path at its own
      quality, the sound up to the node too (simpler and sharper than the picker cells' 320×180);
      Ctrl+Shift+click on a node and the right-click item; the badge over the preview; Esc / Play /
      Ctrl+Shift+click on empty canvas / closing the window return; t2997; live step

## 5. Follow-ups named by his answers (separate changes)

- [ ] 5.0 `track-device-sound` (Q5): a device on a video track sounds on the audio track(s) picked in
      its form (design D9)

## 6. v2 — a separate change, proposed after v1 is in his hands

- [ ] 6.1 propose `fx-graph-dag`: slot ids and edges in the model and `.kt` (a line writes today's
      bytes), Merge/Split, mask and key sockets fed by any node, the DAG in `video_plan` and the audio
      node chain through the one plan, a line graph byte for byte; its OTIO-loss row
