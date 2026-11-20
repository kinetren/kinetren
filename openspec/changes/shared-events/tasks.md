# Tasks: shared-events

> Owner Q1 first. Tests reserved: t3367–t3378. Base: orch `502947e3`.

## 0. Notes first

- [x] 0.1 proposal, design (D1–D7, class walk D6), deltas `project-format`, `transport`; `openspec validate shared-events --strict`

## 1. Measure (Q1 a)

- [ ] 1.1 `kt-cli edit-cost` (release) on a ~2000-event project: the drag line recorded in found-defects 192 and this design

## 2. Shared cells in the model

- [ ] 2.1 `kt_model::shared::Shared<T>` (Deref, `edit`, `edit_if`, pointer-first `PartialEq`, transparent Debug and serde, `fp`); `Cost::EventClone`, `Cost::EventHash`
- [ ] 2.2 the event vectors of `Project`, `Sequence`, `ResolvedReel` become `Vec<Shared<…>>`; every writer through `edit()`
- [ ] 2.3 `simulate`, the equality check, `push_undo`, `rebuild_bench`, `doc_arc`, `reel_document`, the Viewer cut copy pointers
- [ ] 2.4 t3367–t3370; t148, t811, t822, t2004 and the golden tests unchanged; `cargo test -p kt-model -p kt-engine`, clippy, target-prune; commit

## 3. Builders and resolve

- [ ] 3.1 the builders' event keys and the whole-project hashes read `fp()`; `hash_debug` moves into kt-model
- [ ] 3.2 `resolve_reel` passes unreached events as the same cell
- [ ] 3.3 `edit-cost` prints copies and fingerprints per edit; t3371–t3374; the numbers before/after in design D5
- [ ] 3.4 found-defects 192 and 641 closed; the idle measurement on the heavy showcase (pause, pointer, effects window); commit
