# Tasks: export-readback

- [x] 1 The ring by bytes: `readback_depth(w, h)`, `done(ticket)` on `VideoGpuExec` and kt-gpu's ring
      (slots up to the depth, ledger account), the pass timer's pending frames (D1, t2960)
- [x] 2 The compose loop hands on finished frames, waits only on a full ring; the profile's ring and
      latency; `ExportStage` RING/LATENCY (D1, D2, t2961)
- [x] 3 `kt-cli trace … export`: device time a frame, the ring line (old traces: the collect wait), the
      `EXPORT` lines (D2, t2962)
- [x] 4 No `render.log`: `LogFile`, `log_dir`, the path line and its keys go, Open log opens the telemetry
      journal; t959 rewritten; live `render` (D3)
- [ ] 5 found-defects rows 740–741, registry T3/T4, What's new, crate CLAUDE.md; tests, clippy, live,
      target-prune, commit; archive
