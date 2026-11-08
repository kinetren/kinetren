# Tasks

## 1. Data

- [x] 1.1 Every row of `docs/found-defects.md`, the archived performance changes, the design records of transport, audio-render, video-plan, video-fx, media-index and compositions, the owner's claims
- [x] 1.2 A map of every worker, queue, GPU device and cache in the code
- [x] 1.3 Release `kt-cli probe --time` on the relinked haski copy and the heavy showcase; gdb samples of `AudioRenderer::new`

## 2. Deliverables

- [x] 2.1 `docs/engine-audit.md`: mechanisms, replacement vs targeted, scheduler design, JIT, plan, answer
- [x] 2.2 `docs/features/plugins/kernels/fused-kernels.md` (F-EXT-005); `features-index.py --check` green
- [x] 2.3 `docs/found-defects.md` row 151; registry H10/H11 moved; `ktprompts-index.py --check` green

## 3. Close

- [x] 3.1 `openspec validate engine-audit`; archive with `--skip-specs` (no spec delta)
- [x] 3.2 Commit; `scripts/target-prune.py`
