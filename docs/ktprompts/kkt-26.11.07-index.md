# Registry of the owner's list `kkt-26.11.07`

Source — [`kkt-26.11.07.txt`](kkt-26.11.07.txt) (verbatim, the owner's message of 07.11.2026, out of
turn). Columns and states — as in [`v26.10-index.md`](v26.10-index.md). The round is in
`docs/user-feedback.md` ("2026-11-07 — the processor on a pause, out of turn").

## Registry

| # | Line | Claim (brief) | State | Address | Stage |
|---|---|---|---|---|---|
| E1 | 3 | on a pause, with nothing happening, the program takes the whole processor | done idle-cpu: t1830, t1833, live `idle`, 234-244 % → 0-1 % of a core on our heavy project, llvmpipe; on his card to confirm | `idle-cpu` | stage 1 merged |
| E2 | 4 | effects off, the load falls; then just moving the mouse over any static place raises it | done idle-cpu, ui-frame-cost, t1832, t1912, live `idle`: at rest no frame is asked, a hover frame replays the events; the percent on his card is owner-checks' business (the counters are the verdict) | `idle-cpu` | done |
| E9 | 4 | how much a mouse move costs depends on the timeline's scroll position | done ui-frame-cost, t1912, t1913: a frame's work is the view's, not the project's; a hover frame lays out nothing | `idle-cpu` | done |
| E3 | 5 | he has no tool to bring proof; the playback journal is not for this, and he may not even reach it in such lag | done structured-telemetry, load-report-into-trace, t2140, t2571, t2572: one trace of a run, `kt-cli trace`, the load header in it; the Diagnostics button opens its folder | `load_watch.rs` | done |
| E4 | 7 | a reopened project, nothing touched: hovering a composition event with slowed+vhs+isp over it takes the whole processor | done idle-cpu, t1830, t1831: a release an event shows is never evicted, so the previews stop going round | `idle-cpu` | done |
| E10 | 7 | opening the effects of that event takes the whole processor | done idle-cpu, input-without-work, t1871, t1874, live `idle`: the effects window open on a pause costs nothing | `idle-cpu` | done |
| E11 | 7 | why are agents not ordered to test this: a permanent test of load | done idle-cpu, t1830, live `idle` in `finish.sh`, red on the old code; the rule is in the root `CLAUDE.md` | `src/scripts/live/idle.py` | done |
| E5 | 10 | a hint, not a target: the build of 26.11.03 was already bad, and the current one is worse than that bad one. It is no reference for any change | done idle-cpu, t1830: the bar is zero work at rest and no build is a reference; recorded in `docs/orchestrator.md` | every load change | done |
| E6 | 12 | an added effect's preset cannot be changed; it must be deleted and created again | no: feature, change `fx-preset-change`, no note | `fx-preset-change` | on his word |
| E7 | 13 | the Camera ISP form is a sheet three kilometres long, and half of its width is empty; asked to remember for good | done fx-form-width: t3591, t3593, t3593b, t3594, live `form_width` (the ISP sheet 2232 → 758 pt at 1860 pt wide, 1 → 4 block columns); live `idle` with the effects window open stays at 1 % of a core | `fx-form-width`, the guard over every form | merging |
| E8 | 17 | an anomalous parameter (Datamosh as an example) lags the whole interface until the effect cannot be undone; no stupid solutions, the right one | done ui-never-waits: t1840-t1848, live `ui_wait` (Datamosh at its extremes on the heavy showcase: the window, the gallery, Ctrl+Z and Done answer in 0.3-0.5 s, no UI frame noted); on his card to confirm | `ui-never-waits` | merging |
