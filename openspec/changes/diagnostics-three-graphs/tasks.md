# Tasks

Test numbers t1145–t1154 are reserved by this change. Every stage ends with a commit (English,
"diagnostics-three-graphs <stage>: …", no attribution lines) and `src/scripts/target-prune.py`.

## 1. The graph draws from its last sample's build

- [x] 1.1 `graphs.rs`: the auto-scale steps in `push`; a draw cache keyed by (rect, generation) with the lines as one `Arc<Mesh>` and the labels as galleys; a build counter for tests. Verify: t1149 (`graphs.rs`) — repeated draws do not rebuild, a sample and a larger rect rebuild once each, ten draws paint the same scale; on the old code (per-draw scale) the scale assertion is red.

## 2. The perf box moves into Diagnostics

- [x] 2.1 `app.rs`: the left column draws only the bin, and the bin's frame reaches the column's bottom (its background takes the freed space); `draw_perf_box` becomes the "Performance" section of `draw_diagnostics` (minus "Economy UI"); the window body is one scroll, the three graphs share the viewport height left by the log; default 640 × min(720, screen − 60), min 460×320 (t779's 420-point screen); probes `diag.graph.quality|decode|churn`, `diag.log`, `view.diagnostics`, `view.economy`; the graph names its accessibility node (t956). Verify: t1145, t1146, t1147, t1148 (`diag_tests.rs`).
- [x] 2.2 `toolbar.rs`: the View "Economy UI" item takes `hint::ECONOMY`; `perf.economy` removed from the five catalogs; t128 measures `menu.view.economy`. Verify: t128, t125, t131 (font/i18n tests), t1146.
- [x] 2.3 t947, t289 and t720b open the Diagnostics window before reading their lines (the assertions unchanged — the lines moved there); t394 and `scripts/live/rmb.py` comments no longer say the perf panel lives under the bin. Verify: t947, t394.

## 3. Live scenario and documents

- [x] 3.1 `src/scripts/live/diagnostics.py`: the column without the box, the gauge opens the window, playback runs the three graphs, the corner drag enlarges them, View → Diagnostics… opens it again; a `kt_live.step` screenshot per step (at 1900×1000: at 1440 the bottom row cuts the gauge off — a pre-existing toolbar-width defect, not this change). Verify: `KT_LIVE_DISPLAY=:272 ./scripts/live-check.sh diagnostics` — green, six steps; `rmb` (its comment changed) green alone.
- [x] 3.2 `docs/guide/whats-new.md` (## kt26.11) line; `docs/ktprompts/kkt-26.11.01-index.md` row K14 → done; `docs/features/ui/viewer/playback-performance-readout.md` status done; kt-app `CLAUDE.md` perf-panel mentions point at the Diagnostics window; `docs/user-feedback.md` gets the decision. Verify: `python3 src/scripts/ktprompts-index.py --check`, `python3 src/scripts/features-index.py --check`, `python3 src/scripts/specs-user-path.py --check`.

## 4. Finish

- [ ] 4.1 `git merge master`; `cargo test --workspace` and `cargo clippy --all-targets` green; `./scripts/finish.sh` (from `src/`), the version written here. Verify: the finish.sh output.
- [ ] 4.2 Archive the change (specs merged into `ui-timeline`, the why into its `design.md`). Verify: `openspec validate --specs`.
