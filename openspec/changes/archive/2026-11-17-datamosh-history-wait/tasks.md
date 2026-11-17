# Tasks: datamosh-history-wait

- [x] 1 Read the owner's trace (`problem_region_26_11_09/new`): `trace gpu`, the dump of a near job's pieces,
      admissions against their predictions, the present waits (design D0)
- [x] 2 `trace gpu` reads a job's pieces as one job (D1, t2863)
- [x] 3 Every wait names its own submission; `waits_for_all` counts the rest (D2, t2864)
- [x] 4 A ring's waiter looks at every device in flight (D3, t2865)
- [x] 5 The present checks stop growing (D4, t2866)
- [x] 6 Benches on release: showcase with Datamosh, `kt-cli preview` with `--trace`, `trace gpu`, idle load (D5)
- [x] 7 found-defects rows 662, 693; What's new; kt-gpu CLAUDE.md; tests, clippy; commit, target-prune; archive
