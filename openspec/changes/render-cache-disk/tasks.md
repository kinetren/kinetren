## 1. Reconnaissance

- [x] 1.1 Feature note F-EXP-008 `docs/features/export/pipeline/background-prerender.md` (Vegas, Premiere, Resolve, FCP, AE)

## 2. Store and key

- [ ] 2.1 `render_cache::DiskStore`: lossless files, index, cap, LRU, other builds removed, corrupt = miss (D2, t2980)
- [ ] 2.2 `VideoRenderer::probe_durable` and `render_cache::durable_hash`: the plan key's fields without serials, the leaves' identities (D1, t2981)
- [ ] 2.3 `Ev::RenderCache`, `job::PRERENDER` in kt-time (D6)

## 3. Read before composing

- [ ] 3.1 The player's `RenderCache` (root, salt, device folder opened lazily, file identities by state); `Player::set_render_cache`
- [ ] 3.2 Restore in `show_kept` and in the producer's pass before the decoded check; `on_disk` in the bars (D3, t2982, t2985)

## 4. Prerender at rest

- [ ] 4.1 Candidates from the cost map (D4, t2983)
- [ ] 4.2 The `render-cache` thread: quiet, Play, export vote, close; decode at Thumbnail, stream advance, compose at background, measure, write; `note_input` (D4, t2984)
- [ ] 4.3 The scan for the bars at rest (D3, t2985)

## 5. Hosts and trace view

- [ ] 5.1 kt-app: the root, the salt, `note_input` (D7)
- [ ] 5.2 kt-cli: `preview --render-cache DIR`, `trace cache` (D6, D7, t2987)

## 6. Checks

- [ ] 6.1 Measurements: a heavy loop with and without the store, two processes (design Measurements)
- [ ] 6.2 Live scenario `render_cache` (t2988) and the `idle` scenario on a heavy project
- [ ] 6.3 Tests of kt-engine, kt-time, kt-cli, kt-app touched; clippy; target-prune
- [ ] 6.4 found-defects rows, kkt-26.11.17 S5, What's new, crate CLAUDE.md
