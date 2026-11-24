# Tasks: fx-graph-dag

> The owner answered (kkt-26.11.20 A5, A6): Q1 a, Q2 a, Q3 b; the orchestrator's go for v2a on
> 2026-11-20. Tests reserved: t3200–t3229. Base: orch `5e7a8ea9`, branch `claude/fx-graph-v2a`.

## 0. Notes first

- [x] 0.1 proposal, design (D1–D10, the class walk, three questions), spec deltas (`video-plan`,
      `audio-fx`, `morpher`, `project-format`, `ui-reach`); `openspec validate fx-graph-dag --strict`
- [x] 0.2 F-UI-008 gains the branch/merge reconnaissance; `features-index.py --check` green
- [x] 0.3 his answers Q1–Q3 in `docs/user-feedback.md` (orchestrator) and folded into this design
      (revision 2: D2 "Reads" replaces the trunk rule; `MAX_READS`; t3226–t3229)

## 1. v2a — the model and the file

- [x] 1.1 kt-model: `fx_in` (`SlotInputs`, `FxIn`, `MatteFrom`) on every chain carrier (event,
      track, morpher patch's two chains); `FxInstance::Merge` (core, not in the plugin registry)
- [x] 1.2 validation: earlier slots only, one lane, `second`/`matte` only on Merge and a compressor's
      key, the lead before any branch, `MAX_HELD`, `MAX_READS`; the stable topological re-sort for a
      forward wire; t3209
- [x] 1.2b `fx_reads`: a graph's reads as derived chains (ancestors of an arm time kind, nested);
      a graph without arm time kinds has one read equal to its Output path; t3209b (the engine's
      order and strip per read stay t3227, v2b)
- [x] 1.2c the pair's speed is the trunk's: an arm's warp counts as none for the link group, a
      rewire re-runs the mirror; one warp slot per chain stays (D11.5); t3209c
- [x] 1.2d until v2b a Merge is inert (it leaves the active chain like a missing plugin, its
      readers reading its Background): a graph ships only with its lowering
- [x] 1.3 one removal helper re-wiring readers; the morpher splice offsets inputs; the pair rules
      mirror presence and values, not wires; t3222, t3223
- [x] 1.4 copy/paste blocks and presets carry relative inputs (`append_chain_block`); t3224
- [x] 1.5 `.kt`: `in`, `merge`, format by content (the next free number at merge); t3210
- [x] 1.6 kt-otio: `in` in `Effect.1` metadata, `kt:merge`, envelope v18 by content; t3211; the row
      in "What OTIO export loses" (`docs/ktprompts/v26.10-index.md`)

## 2. v2b — the picture

- [x] 2.1 kt-fx: lowering in `plan_chain_in` (`Hold`/`Load`/`Merge` as stops, liveness); t3203
- [x] 2.2 kt-gpu: the held set, the Merge pass (blend kernels + `LayerMix` with a coverage), the
      `matte_from` pass; alpha flags per side; t3200, t3201, t3202, t3204
- [~] 2.3 kt-engine: decode step only before the first branch (done: `clip_head_zoom` reads the
      trunk); masks' dry side = the slot's input (done in `chain_ops`: a graph step stands between
      a mask's close and the next slot's open); t3205, t3206 to write
- [x] 2.3a `clip_warp` and the stream stage read the TRUNK's time kinds only (the picture's
      `chain_context` is the sound's, v2c); a Merge is a slot of the active chain again; the lead
      enters the wiring by `fx_graph::insert_lead`
- [x] 2.3b kt-engine reads: `VideoClip.reads` (a leaf's arm time kinds, each a line clip of the same
      event), `layers_at` orders every read, `LayerSrc.reads` resolved and encoded first by kt-gpu,
      `PlanOp::Input` at the slot, the export chunk counts them, both keys hash them; a read is a line
      (`ReadBranches`); the stream history keys by content, so a read's own front has its own; the
      read law t3226, the order t3227. Open: t3228 (Datamosh in an arm), the stream memo's want and
      `stream_advance_run` for a read's stage
- [ ] 2.3c kt-app: the coverage strip draws a row per read; the performance panel and `kt-cli probe`
      name the reads
- [x] 2.4 keys: `fx_chain_hash`, `plan_hash`, `durable_hash`, post-FX key hash `fx_in` when present;
      t3208
- [ ] 2.5 Viewer by ancestors; picker cells insert on the selected node's arm; t3207
- [ ] 2.6 pass-cost rows, prewarm ids, the ledger counts held frames
- [ ] 2.7 `kt-cli probe` prints the arms per event

## 3. v2c — the sound (after v2b, Q1 a)

- [x] 3.1 `ClipProcessor` routes held outputs (`audio/graph.rs`: spill before the working buffer
      is overwritten, load, the Output loaded last); Merge Sum/Crossfade; kept register buffers;
      the track stage runs the same pass (`TrackProcessor::new_graph`); t3212, t3213
- [x] 3.2 look-ahead alignment before a Merge and a keyed node (a ring per input that has less);
      the chain's look-ahead is the Output path's (`chain_lookahead_in`); t3214
- [x] 3.3 the compressor's key socket from its own chain (`FxInstance::has_key_socket`, a
      `second` on a keyed kind passes validation; the wired key wins over a track key); t3215
- [x] 3.4 tails of the Output path, dangling nodes not run (`sound_split` leaves them as
      placeholders); `chain_fingerprint_in` hashes the wiring; t3216, t3217
- [x] 3.5 sound reads: `sound_split` makes each read a line, `FxStateStore` keeps a processor
      per read, `EventFeed` reads it through its own `SrcWindow`, the prefetch asks each read's
      window; a reel instance opens no read (transparent, as the picture); t3229
- [x] 3.6 the sound lane takes wiring (`SoundLater` gone); a Matte on a sound chain is refused
      (`MatteOnSound`)

## 4. v2d — the editor

- [ ] 4.1 `node_editor`: arms as rows under the trunk, wires in the gaps, Merge sockets
- [ ] 4.2 gestures: a wire into empty canvas starts an arm; Merge first in the picker; connect and
      disconnect; a time kind in an arm marked as a second read; refusals with reasons; t3218,
      t3219, t3220; remove `SetFxIn`/`SetTrackFxIn` from t252's `NOT_FROM_UI`
      (v2a put them there); every chain edit of the editor keeps the wiring (`remove_slot`,
      `insert_slot`) so a format-8 file's slot can be deleted (found-defects 903)
- [ ] 4.3 i18n ×5 (Merge, its modes and sockets, the refusals); `subset-fonts.py` if needed
- [ ] 4.4 idle with a branched graph on the heavy showcase; t3221
- [ ] 4.5 live `node_graph_branch` (a screenshot per step); live `idle`, `node_graph`, `fxwin`
- [ ] 4.6 What's new line; `docs/guide` page on the node editor gains branches; U3's registry row

## 5. Close

- [ ] 5.1 the specs' tests, `cargo test` of touched crates, clippy, live scenarios, target-prune;
      found-defects rows for every cause found; commit per stage
