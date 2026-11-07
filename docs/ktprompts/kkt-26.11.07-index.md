# Registry of the owner's list `kkt-26.11.07`

Source — [`kkt-26.11.07.txt`](kkt-26.11.07.txt) (verbatim, the owner's message of 07.11.2026, out of
turn). Columns and states — as in [`v26.10-index.md`](v26.10-index.md). The round is in
`docs/user-feedback.md` ("2026-11-07 — the processor on a pause, out of turn").

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| E1 | 3 | on a pause, with nothing happening, the program takes the whole processor | done idle-cpu: t1830, t1833, live `idle`, 234-244 % → 0-1 % of a core on our heavy project, llvmpipe; on his card to confirm | `idle-cpu` | stage 1 merged |
| E2 | 4 | effects off, the load falls; then just moving the mouse over any static place raises it | partial: idle-cpu, a mouse move costs repaints and no work (t1832); the frame on llvmpipe stays 16 ms, on a card not measured | `idle-cpu` | stage 1 merged |
| E9 | 4 | how much a mouse move costs depends on the timeline's scroll position | partial: idle-cpu, the walks over events cut; a dependence on scroll is not measured apart | `idle-cpu` | stage 1 merged |
| E3 | 5 | he has no tool to bring proof; the playback journal is not for this, and he may not even reach it in such lag | partial: idle-cpu, `load-report.txt` and the Diagnostics button (t1839), not yet seen by him | `load_watch.rs` | stage 1 merged |
| E4 | 7 | a reopened project, nothing touched: hovering a composition event with slowed+vhs+isp over it takes the whole processor | partial: idle-cpu, the composition previews stop going round (t1830); needs 7 or more different releases to fire, his project unknown | `idle-cpu` | stage 1 merged |
| E10 | 7 | opening the effects of that event takes the whole processor | partial: idle-cpu, 202-274 % → 2 % with the window open on a pause (live `idle`); on his card to confirm | `idle-cpu` | stage 1 merged |
| E11 | 7 | why are agents not ordered to test this: a permanent test of load | partial: idle-cpu, the live scenario `idle` with limits runs in `finish.sh`, red on the old code (t1830) | `src/scripts/live/idle.py` | stage 1 merged |
| E5 | 10 | a hint, not a target: the build of 26.11.03 was already bad, and the current one is worse than that bad one. It is no reference for any change | no: defect, `transport`, change `idle-cpu`: the bar is zero work at rest, not that build | every load change | on his word |
| E6 | 12 | an added effect's preset cannot be changed; it must be deleted and created again | no: feature, change `fx-preset-change`, no note | `fx-preset-change` | on his word |
| E7 | 13 | the Camera ISP form is a sheet three kilometres long, and half of its width is empty; asked to remember for good | no: defect, `ui-reach`, change `fx-form-width` | `fx-form-width`, the guard over every form | on his word |
| E8 | 17 | an anomalous parameter (Datamosh as an example) lags the whole interface until the effect cannot be undone; no stupid solutions, the right one | no: defect, `transport`, change `ui-never-waits` | `ui-never-waits` | out of turn, on his word |
