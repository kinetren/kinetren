# Tasks: project-scale

> Waits for Q70 (the budget) and Q71 (the table); task 1 (`kt-cli scale`) and D2 need neither. Tests: t3924–t3931.

## 0. Notes

- [x] 0.1 proposal, design (D1–D6), delta `project-format`; `openspec validate project-scale --strict`
- [ ] 0.2 the owner's answers folded in

## 1. Measure

- [ ] 1.1 `kt-cli scale` (shapes, sizes, the nine paths); t3929
- [ ] 1.2 the release table at 10 k / 100 k / 1 M into design revision 2; found-defects rows (block 1100–1149) for every path over the budget or non-linear

## 2. File

- [ ] 2.1 the single typed parse with the version peeked; the borrowing envelope; t3924, t3925
- [ ] 2.2 the chain table on write, expansion on read, the version by content; t3926, t3927
- [ ] 2.3 OTIO export expands; t3928
- [ ] 2.4 growth guards; t3930, t3931
- [ ] 2.5 the table re-measured after 2.1–2.2 (the owner's 10 k × 100 case: size and read time)

## 3. Close

- [ ] 3.1 What's new (a project with many copies saves smaller and opens faster); registry C2a, C2c moved
- [ ] 3.2 `src/scripts/finish.sh` (orchestrator), version in the report
