# Tasks

## 1. Diagnosis

- [x] 1.1 Read the owner's traces (`rendereg_2`, `problem_region_26_11_09`, `rendereg`, `26.11.14/2`) with `kt-cli trace during|loop|gpu|dump`; the mechanism and the per-subsystem table are in design.md (Context, class walk)

## 2. The fill's width

- [x] 2.1 `VideoGpuExec::sibling_exec` (default `None`), kt-gpu's engine returns its `sibling`; `cargo build -p kt-engine` green
- [x] 2.2 `loop_fill.rs`: lanes per moment in flight, claims, `width()` from `sched::workers()` and the device kind, the display gate on software only, `narrow` on an incomplete moment, `KT_LOOP_FILL_WIDTH`; t2861 (width rule)
- [x] 2.3 `Player::fill_width`, `composing_for_tests`; `kt-cli preview` prints the width
- [x] 2.4 t2860 (several moments in flight on a card, no moment twice, faster than width 1), t2862 (an incomplete moment narrows the fill for the lap); t2334–t2336 still green (`cargo nextest run -p kt-engine --test loop_fill`)

## 3. Measured and recorded

- [x] 3.1 Release bench on llvmpipe, width 1 against forced width 3, per-thread load (design D4)
- [x] 3.2 found-defects rows 693, 694 (what the class walk found and left)
- [ ] 3.3 Idle cost on a heavy project: live scenario `idle` (pause, the pointer over a static place, the effects window open) stays at rest
- [ ] 3.4 `cargo test --workspace` and `cargo clippy --all-targets` green; target-prune
- [ ] 3.5 What's new line (`docs/guide/whats-new.md`, `## kt26.11`), the registry row N4 of `kkt-26.11.15-index.md`
- [ ] 3.6 Archive: specs merged, the "why" in `openspec/specs/video-plan/design.md` (finish.sh is the orchestrator's, per stage)
