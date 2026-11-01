# Tasks

Test numbers t1105–t1114 are reserved by this change. Every stage ends with a commit (English
message, the first line = the change, the stage and what was done) and `src/scripts/target-prune.py`.

## 0. Records before code

- [x] 0.1 The owner's claim (kkt-26.11.01 item 15) is in `docs/user-feedback.md` (2026-11-01) and the registry row K15; `pitch-loop-silence` archived first (its remaining tasks were finish and the design "why"). This change's artifacts written. Verify `openspec validate pitch-event-edges --strict` is green
  - Measured before any fix (`pitch_edges.rs` probe, 15 × 100 ms split of a 440 Hz sine): WSOLA +7 —
    ≈ 320 frames of total silence before every join, −7 — ≈ 250 frames after it, step 0.46 against
    0.004 uncut; vocoder — level 0.42 of uncut at the joins, step 0.68 against 0.006.

## 1. Prove the cause

- [x] 1.1 Write t1105 (split == uncut, byte for byte, slicings) and t1106 (other material: no dip) in `src/libs/kt-engine/tests/pitch_edges.rs`. Verify both RED with the numbers of 0.1 before stage 2
  - RED: t1105 "Wsola +7: the split differs from the uncut event (worst dip 0.000 of the steady
    level, step 0.4593 against 0.0041 uncut)"; t1106 "Wsola +7: a window at a join fell to 0.000".

## 2. Context across the edges (D1, D2)

- [x] 2.1 `Decl.reads_across_edges` (kt-plugin-api, default false), pitch declares it; `AudioNode::reads_across_edges`; `kt_fx::chain_context(fx, auto)` — the one home. Verify t322 (registry frame) green
- [x] 2.2 `ClipProcessor`: the context flag from `chain_context` (apply_params/set_auto); `hard_seek` pre-roll unclamped, the input gate at `gate_end + la`, the content gate after the last edge-reading node (and the waiting dry side); `before_material` only for a FOLDED input. The window functions extend `material` by the pre-roll and `La` under the context. Verify t1106, t1107, t1108, t1109 green; t334–t340, t906–t911, t17, t336, t973c green unchanged
  - Green. t973c: ONE frozen value moved — pitch_shift "at 70 %" (0x6a8e26e7182e4946 →
    0x3c85d210d0eb2c6d): that event spans the whole render and its material goes on past the end,
    so its last grains now read it instead of silence — the change asked for; its default, curve and
    reversed cases did not move. Re-printed with a comment naming this change.
- [x] 2.3 Commit stage 2
  - Committed together with stage 3: t1105 (split == uncut) needs both, and `cargo test` must be green on every commit

## 3. One stream across a split (D3, D4, D5)

- [x] 3.1 kt-engine: the run predecessor per event (`build_reel`, after the overlaps); `FxStateStore::hand_on`; `ClipProcessor::hand_on` (rebase) and the fresh-entry predicate; `render_fx_clip` hands on or lets the processor seek; the loop span only for an event that also covers the region start when the context applies. Verify t1105, t1110, t1111, t1112 green
  - Green, both engines. t1111 found its own mistake twice: a 1-frame gap rounds away in the tick
    grid (1 frame = 1.875 ticks) — 8 frames (15 ticks) are used; a skipped source breaks the join
    after the event too.
- [x] 3.2 `src/libs/kt-fx/CLAUDE.md` look-ahead node rules (reads across edges, the content gate, the hand-off) and `src/libs/kt-engine/CLAUDE.md` (the run predecessor). Verify `cargo test --workspace` and `cargo clippy --all-targets` green (no new warnings); commit stage 3
  - `cargo test --workspace --no-fail-fast` under three parallel sessions: red t815 and t663 (timed,
    green alone) and t476, t411 (the index DB service in the Bash sandbox; green outside it, 38 s).
    Clippy: one new warning (an unused test helper) removed; the rest are not from this change.

## 4. Finish

- [x] 4.1 What's new line (`docs/guide/whats-new.md`, `## kt26.11`); registry row K15 moved; `python3 src/scripts/ktprompts-index.py --check` green
- [ ] 4.2 Run `./scripts/finish.sh` from `src/` (`KT_LIVE_DISPLAY=:233`). Verify green and record the version here; ask the owner to re-listen to his fifteen events on that build (and whether they are cut from one recording)
- [ ] 4.3 Archive; append the "why" (edges read the event's own source, the content gate, one stream across a split, a fresh entry is a seek) to `openspec/specs/audio-fx/design.md` and `openspec/specs/audio-render/design.md`
