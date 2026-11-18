# Registry of the owner's list `kkt-26.11.17`

Source — [`kkt-26.11.17.txt`](kkt-26.11.17.txt) (verbatim, one message of 17.11.2026 after stage 17,
release kt26.11.16-c131e483). Columns and states — as in [`v26.10-index.md`](v26.10-index.md).

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| S1 | 1 | start further work slowly as `datamosh-history-wait` and `decode-waste` finish | partial: queue — found-defects 710 (export device count), 697 (source colour matrix on decode), 680–685 | `datamosh-history-wait`, `decode-waste` | 18 |
| S2 | 2 | remind him of the trace request in about 7 and 17 hours | partial: two session reminders set | — | 18 |
| S3 | 2 | he is tired of recording traces for us: a separate agent to think whether a fundamental (breaking) change or a different architecture is needed | partial: `render-ahead` done (review [`architecture-review-26.11.17.md`](../architecture-review-26.11.17.md): GPU scheduling of the preview is the structural weak point; his four answers S4–S7) | `docs/architecture.md` | 18 |
| S4 | 5 | raise the half-frame bound of GPU work in flight | done render-ahead: the bound is one frame period for every lower class (t2923, t2433), and playback is a render-ahead pipeline with a measured depth (t2920, t2921, t2925; live scenario `render_ahead`); the gain on a card waits for his trace or the reference box (S7) | `video-plan` | 19 |
| S5 | 6 | the disk prerender of slow regions runs by itself in the background | done `render-cache-disk` (review R3): moments measured at least half a frame period are prerendered at rest after 1.5 s with no input, kept on disk under a durable plan key, played from it after a restart; t2980–t2985, t2987 (live `render_cache`) | `video-plan`, `telemetry` | 21 |
| S6 | 7 | the processor as a second executor for the export AND the preview; frames within tolerance, not byte for byte | no → `cpu-second-executor` (review R4), after `render-ahead`; amends invariants 7 and 11 | `video-plan`, `export` | 19+ |
| S7 | 8 | buy a mini-PC with an AMD iGPU as the reference machine | future: his purchase; the nightly bench (review R1, R10) is prepared to run on it | — | — |
