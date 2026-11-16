# Tasks: render-speed

- [x] 1 Diagnosis on release: the plain and heavy showcase, export against preview at 1080p and at 640×360,
      one against several software devices, the arbiter A/B (design D0)
- [x] 2 The export keeps its profile always and writes it into the trace (`Ev::ExportStage`) and the journal
      line (`ExportStats::stages`); the single-device stages (D2, t2792)
- [x] 3 `kt-cli trace DIR export` (D3, t2791); `export --range/--size`, `gpu-share --mosh none`
- [x] 4 found-defects rows; What's new; tests, clippy; commit, target-prune; archive
