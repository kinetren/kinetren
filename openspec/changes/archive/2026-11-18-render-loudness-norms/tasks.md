# Tasks

Tests t3106–t3107 (the `features-slow` block). Branch `render-loudness-norms` from `orch-26-11-01`.

## 1. The meter

- [x] 1.1 Feature note `docs/features/export/pipeline/render-loudness-report.md` (F-EXP-009)
- [x] 1.2 `ChannelLoudness` per channel over the summed gate's blocks; the true-peak gate per channel
- [x] 1.3 t3106 green; t403–t405 unchanged and green; kt-fx 79/79

## 2. The table

- [x] 2.1 `loudness_rows` (norms, one place) and `loudness_table` in the result block; the `render.loudness`/`loudness_to_target` keys replaced by fifteen `render.loud_*` keys in five catalogs
- [x] 2.2 t3107 green
- [x] 2.3 The progress window opens at 600 (the channels row was under the fold at 520 — seen in the live run)

## 3. Live, docs, close

- [x] 3.1 Live scenario `scripts/live/render_loudness.py`: a left-only tone, the table with "R silent" in red
- [x] 3.2 kt-engine export loudness 2/2; UI test groups green after the Chinese font subset was rebuilt (two new glyphs, t100) and the scenario's docstring quote put in English (t933)
- [x] 3.3 What's new; registry T14 → done, T12 → done (closed by render-window-v1-parity, registry stale); F-EXP-009 done; archive
