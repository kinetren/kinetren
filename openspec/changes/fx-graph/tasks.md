# Tasks: fx-graph

> Nothing below starts before the owner's go and his answers to O1–O5 (`design.md`). Tests reserved:
> t2990–t3009.

## 0. Notes first

- [x] 0.1 F-UI-008 `docs/features/ui/editing/fx-chain-graph.md`; editor keys `bitwig-6`, `reaper-7`;
      `features-index.py --check` green
- [x] 0.2 proposal, design (mockups, gestures, class walk, open decisions), spec deltas (`morpher`,
      `audio-fx`, `ui-reach`); `openspec validate fx-graph --strict` green
- [x] 0.3 registry `kkt-26.11.18b-index.md` row U3 → partial, `fx-graph`
- [ ] 0.4 the owner's answers O1–O5 written into `docs/user-feedback.md` and into this design before
      stage a

## 1. The patch holds both halves (stage a — the defect)

- [ ] 1.1 kt-model: one n-th-occurrence matcher shared by `sync_pairs_to_link_group` and a new
      `sync_pairs_in_patch`, called from `SetMorpher` (edited side from the command; leader rule for
      model-only callers)
- [ ] 1.2 remove `MorphPatch::derived_pairs`; `chain_for`/`auto_for` return own enabled slots (warp
      and strength unchanged)
- [ ] 1.3 `Project::normalize`: materialize the halves of older patches (the only migration place)
- [ ] 1.4 tests t2990 (model), t2991 (old file byte-identical render), t2992 (picture untouched by the
      sound half's switch, both-on equals an event-carried device); rewrite t1461b per its new text
- [ ] 1.5 kt-app, without the graph yet: the morpher strip shows the sound half (it is now a slot) —
      the owner's defect is closed even if stage b waits; t2993 on the strip, then on the graph
- [ ] 1.6 the OTIO-loss row of sim-audio-pairs extended; kt-model CLAUDE.md "A simulated device…"
      paragraph; What's new line; commit; `target-prune.py`

## 2. The graph widget (stage b)

- [ ] 2.1 `fx_graph.rs`: rails, nodes, ties, head/lead, `+` gaps with the picker, drag within a rail,
      `×`, on-dots, copy-paste and presets moved from `chain_strip`, `ChainCtx` right click
- [ ] 2.2 layout cache keyed by revision/width/selection/folds/hover; one mesh; hover re-tint only
- [ ] 2.3 i18n ×5 (rail names, refusal hint, group labels), font subsets if new glyphs
- [ ] 2.4 tests t2994, t2995, t3001; `reach_tests.rs`, `layout_tests.rs` (t244/t218 per O2),
      `font_tests.rs`/`i18n_tests.rs`; commit; prune

## 3. The three windows (stage c)

- [ ] 3.1 FX window: anchor + partners as rails; the forms below unchanged
- [ ] 3.2 morpher window: two vectors as two rails (or one lane per O2), head before both
- [ ] 3.3 track window: its rail, the key wire to the key track's compressor
- [ ] 3.4 the context groups (layers bottom-up, track stage) per O3; t2996
- [ ] 3.5 remove `widgets::chain_strip`; the live scenarios that clicked crumbs follow the graph
      (`vhs_sound`, `stream_sound`, `camera_mic`, morpher ones); new live `morph_pair`
- [ ] 3.6 cost at rest: t3003, live `idle` with both windows open on the heavy showcase (t3006);
      load measured on a pause, hover, and with the windows open — numbers in the report
- [ ] 3.7 specs merged, What's new, commit; prune; the orchestrator runs `finish.sh` on the stage
