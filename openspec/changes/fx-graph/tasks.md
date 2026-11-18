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

- [ ] 2.1 `node_editor.rs`: canvas (fit on open, Home, wheel zoom at pointer, middle-drag pan,
      collapse below readable zoom, H), layout by order, sockets by kind, Source/Output pinned
- [ ] 2.2 nodes: effect, device (two lanes, two switches, crossing → two tied half-nodes), group
      (layer, composition, track stage; Tab/Esc, breadcrumb), key wire
- [ ] 2.3 gestures: link-drag into empty canvas → picker at the pointer; Shift+A; drop on a wire
      inserts; M, X, Ctrl+C/V (the existing blocks), presets, right-click target; refusals with a
      reason (branch, second input, cross-kind, cycle)
- [ ] 2.4 layout cache by (revision, size, zoom, pan, selection, collapsed set, entered group); one
      mesh; hover re-tint only
- [ ] 2.5a kt-plugin-api `UiDecl.main`/`main_sound` (≤ 3 rows; a device must name them; the fallback
      for the rest); the main sets of ISP, VHS Tape, Datamosh; the node's sliders through the common
      row; bounded node height; sliders fold before the node does; t2998, t2999
- [ ] 2.5 i18n ×5 (socket kinds, refusals, breadcrumb, group labels), font subsets if new glyphs
- [ ] 2.6 tests t2994, t2995, t3001; `reach_tests.rs`, `layout_tests.rs`, `font_tests.rs`,
      `i18n_tests.rs`; commit; prune

## 3. v1c — the windows

- [ ] 3.1 FX window: the event and its partners in one editor, the form beside it (split remembered)
- [ ] 3.2 morpher window: the purple Source-swap node, both vectors as lanes (t244/t218 rewritten)
- [ ] 3.3 track window (key wire into the compressor's key socket); a composition instance's chain
- [ ] 3.4 context groups: read-only boxes for layers and the track, Tab peeks, double click opens; t2996
- [ ] 3.5 remove `widgets::chain_strip`; the live scenarios that clicked crumbs follow the editor
      (`vhs_sound`, `stream_sound`, `camera_mic`, morpher ones); new live `morph_pair`
- [ ] 3.6 cost at rest: t3003, live `idle` with both windows open on the heavy showcase (t3006); load
      on a pause, hover and with the windows open — numbers in the report
- [ ] 3.7 specs merged, What's new, commit; prune; the orchestrator runs `finish.sh` on the stage

## 4. v1d — the Viewer

- [ ] 4.1 the plan takes a cut point for one event (a shorter chain through the one plan builder);
      Ctrl+Shift+click, badge, Esc; t2997; commit; prune

## 5. Follow-ups named by his answers (separate changes)

- [ ] 5.0 `track-device-sound` (Q5): a device on a video track sounds on the audio track(s) picked in
      its form (design D9)

## 6. v2 — a separate change, proposed after v1 is in his hands

- [ ] 6.1 propose `fx-graph-dag`: slot ids and edges in the model and `.kt` (a line writes today's
      bytes), Merge/Split, mask and key sockets fed by any node, the DAG in `video_plan` and the audio
      node chain through the one plan, a line graph byte for byte; its OTIO-loss row
