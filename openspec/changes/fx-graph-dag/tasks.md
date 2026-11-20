# Tasks: fx-graph-dag

> DESIGN ONLY until the owner answers Q1–Q3 (`design.md`) and the orchestrator gives the go. Tests
> reserved: t3200–t3229. Base: orch `3488fbe1`.

## 0. Notes first

- [x] 0.1 proposal, design (D1–D10, the class walk, three questions), spec deltas (`video-plan`,
      `audio-fx`, `morpher`, `project-format`, `ui-reach`); `openspec validate fx-graph-dag --strict`
- [x] 0.2 F-UI-008 gains the branch/merge reconnaissance; `features-index.py --check` green
- [ ] 0.3 his answers Q1–Q3 in `docs/user-feedback.md` (orchestrator) and folded into this design

## 1. v2a — the model and the file

- [ ] 1.1 kt-model: `fx_in` (`SlotInputs`, `FxIn`, `MatteFrom`) on every chain carrier (event,
      track, morpher patch's two chains); `FxInstance::Merge` (core, not in the plugin registry)
- [ ] 1.2 validation: earlier slots only, one lane, `second`/`matte` only on Merge and a compressor's
      key, the trunk-only kinds, `MAX_HELD`; the stable topological re-sort for a forward wire; t3209
- [ ] 1.3 one removal helper re-wiring readers; the morpher splice offsets inputs; the pair rules
      mirror presence and values, not wires; t3222, t3223
- [ ] 1.4 copy/paste blocks and presets carry relative inputs (`append_chain_block`); t3224
- [ ] 1.5 `.kt`: `in`, `merge`, format by content (the next free number at merge); t3210
- [ ] 1.6 kt-otio: `in` in `Effect.1` metadata, `kt:merge`, envelope v17 by content; t3211; the row
      in "What OTIO export loses" (`docs/ktprompts/v26.10-index.md`)

## 2. v2b — the picture

- [ ] 2.1 kt-fx: lowering in `plan_chain_in` (`Hold`/`Load`/`Merge` as stops, liveness); t3203
- [ ] 2.2 kt-gpu: the held set, the Merge pass (blend kernels + `LayerMix` with a coverage), the
      `matte_from` pass; alpha flags per side; t3200, t3201, t3202, t3204
- [ ] 2.3 kt-engine: decode step only before the first branch; masks' dry side = the slot's input;
      t3205, t3206
- [ ] 2.4 keys: `fx_chain_hash`, `plan_hash`, `durable_hash`, post-FX key hash `fx_in` when present;
      t3208
- [ ] 2.5 Viewer by ancestors; picker cells insert on the selected node's arm; t3207
- [ ] 2.6 pass-cost rows, prewarm ids, the ledger counts held frames
- [ ] 2.7 `kt-cli probe` prints the arms per event

## 3. v2c — the sound (Q1 decides whether it ships with v2b)

- [ ] 3.1 `ClipProcessor` routes held outputs; Merge Sum/Crossfade; pool buffers; t3212, t3213
- [ ] 3.2 look-ahead alignment before a Merge (the dry ring generalised); t3214
- [ ] 3.3 the compressor's key socket from its own chain; t3215
- [ ] 3.4 tails of the Output path, dangling nodes silent; `chain_fingerprint` hashes `fx_in`;
      t3216, t3217

## 4. v2d — the editor

- [ ] 4.1 `node_editor`: arms as rows under the trunk, wires in the gaps, Merge sockets
- [ ] 4.2 gestures: a wire into empty canvas starts an arm; Merge first in the picker; connect and
      disconnect; refusals with reasons; t3218, t3219, t3220
- [ ] 4.3 i18n ×5 (Merge, its modes and sockets, the refusals); `subset-fonts.py` if needed
- [ ] 4.4 idle with a branched graph on the heavy showcase; t3221
- [ ] 4.5 live `node_graph_branch` (a screenshot per step); live `idle`, `node_graph`, `fxwin`
- [ ] 4.6 What's new line; `docs/guide` page on the node editor gains branches; U3's registry row

## 5. Close

- [ ] 5.1 the specs' tests, `cargo test` of touched crates, clippy, live scenarios, target-prune;
      found-defects rows for every cause found; commit per stage
