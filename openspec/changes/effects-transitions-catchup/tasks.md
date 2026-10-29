# Tasks

Test numbers t978–t983 are reserved by this change. It depends on `plugin-api-stage1` for the
plugin form of every wave; stages 0–1 do not wait for it. Every stage ends green and is committed
at once (English message, first line = the stage and what was done).

## 0. Records before code

- [x] 0.1 The owner's words (item 6 of `kkt.txt`, "plugins before effects") and the ktprompts FX items are recorded in `docs/user-feedback.md`, round of 2026-10-27/28; this proposal recorded
- [x] 0.2 The reconnaissance note F-FX-007 (`docs/features/fx/catalogue/market-catalogue-and-waves.md`) and the `capcut-2026` row in `docs/features/_editors.md` are written
- [x] 0.3 The `fx/` coverage row of `docs/features/README.md` (6 / 6 → 7 / 7) and the summary-table row for F-FX-007 (inserted from `features-index.py --table`); the new subcategory `fx/catalogue/` named in the README's category prose. Verify `python3 src/scripts/features-index.py --check` green and `openspec validate effects-transitions-catchup --strict` green; commit stage 0
  - Note: the F-FX-007 summary row and the category text had already landed with the coordinator's README commit 1314b9b (row appended at the end of the table, coverage left at 7 / 6); this stage set the coverage to 7 / 7 and added the `fx/` subcategory paragraph naming `catalogue/`. The rest of the hand-kept summary table has drifted from `--table` in other notes' rows (titles, order) — not touched here

## 1. The guards

- [ ] 1.1 Write `src/apps/kt-cli/tests/fx_catalogue.rs` with t978 (lists equal the registries, counts in the headings), t979 (ids and capabilities exist), t980 (verdict vocabulary, `[NOT VERIFIED]` on unchecked rows), t981 (the closed list of rejected names → a `Revision:` line), t982 (wave order, families, capability per wave), t983 (the owner's items). Each guard first runs on a broken temporary copy of the note/design and must be red with the named culprit, then on the tree and must be green
- [ ] 1.2 Verify `cargo test -p kt-cli --test fx_catalogue` and `cargo test --workspace` green; commit stage 1

## 2. After `plugin-api-stage1`

- [ ] 2.1 Switch t978 from `FxInstance::all()` / `TransitionKind::ALL` / `GeneratorKind` to the assembler's closed list plus the core-owned kinds; verify t978 green and that a plugin registered through the test door is NOT required in the note
- [ ] 2.2 Open the change for W1 ("Lens and camera") with `/opsx:propose`, starting from D2's W1 block; its reconnaissance is a live re-check of the lens group (F-FX-007 rows marked `[NOT VERIFIED]`). Verify the new change validates and t979/t982 still pass

## 3. Finish

- [ ] 3.1 Run `./scripts/finish.sh` from `src/`; record the version `vYY.MM.DD-hash` in the report
- [ ] 3.2 After archiving: move D2/D3 (the waves and the rank) into `openspec/specs/video-fx/design.md`, point t979/t981 there, and set F-FX-007's verdict line `CAPABILITY: openspec/specs/video-fx/`
