# Tasks: durable-history-and-mix

> The owner decided (kkt-26.11.20 A14): build every stage now, the history with LZ4, both tiers in the
> project's cache folder — after `project-cache-folder` stage 1. Tests reserved: t3336–t3355. Revised on
> orch `5e7a8ea9`.

## 0. Notes first

- [x] 0.1 proposal, design (D1–D8, class walk D7), deltas `video-fx`, `audio-render`, `telemetry`;
      `openspec validate durable-history-and-mix --strict`
- [x] 0.2 his answers: Q1 b, Q2 b (kkt-26.11.20 line 19); design D9 answers his question; rows 770, 771
      point here

## 1. Measure a restart (telemetry, not a gate)

- [ ] 1.1 the restart summary records (history positions walked, device time; covered bursts rendered
      again); `trace summary` line; t3336
- [ ] 1.2 `render_cache` record kinds for stream entries and mix runs; `trace cache`; t3337

## 2. The history tier

- [ ] 2.1 the persistent cache's `history/` kind and the `mix/` tenant of the cache folder (`project-cache-folder` D1, D3); t3344, t3345
- [ ] 2.2 kt-plugin-api: `EntryCodec`, `StreamMemo` backing (holds/get consult the index; pinned kept)
- [ ] 2.3 Datamosh's codec of `Planes` (exact raw planes); LZ4 in the host's writer (`lz4_flex`), the ratio
      recorded per write; t3339
- [ ] 2.4 the stage's durable salt (`video_stream.rs`, the files of the stage, build, codec version); t3340
- [ ] 2.5 writes at rest on the prerender thread; live checkpoints read back at rest only; t3338, t3341,
      t3342, t3343
- [ ] 2.6 `cargo test` kt-plugin-api, kt-engine, fx-video-codec; clippy; idle-load check with the heavy
      showcase (at rest after the writes: no work); target-prune; commit

## 3. The mix tier

- [ ] 3.1 the snapshot of unwritten units through `MixCoverage`; writer at rest; runs and manifests
- [ ] 3.2 the loader on the prefetch thread, the bounded channel, the drain at a chunk's start; t3346–t3350
- [ ] 3.3 the bars' scan at rest; t3351; the export does not read it; t3352
- [ ] 3.4 kt-engine `CLAUDE.md` (the render thread never reads a file; a new kept result names its durable
      salt), What's new line ("a restart no longer recomputes Datamosh history and the covered sound")
