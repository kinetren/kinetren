# Tasks: windows-fill-their-area

## 1. The shared window rule

- [x] 1.1 `window_shell.rs`: resizable by default, kind default sizes, `.remember(family)` with the
      persisted family size (D1, D3); drop `.resizable()`; every window site gives a fitting
      `.size(…)` or relies on the kind default
- [x] 1.2 `widgets.rs`: `window_body` fills (no 0.62× cap), `window_fill` without the action row,
      `fill_probe` records the body rect per layer, `popup_list_height` (D2, D6)
- [x] 1.3 window sites: notices, search, analysis, dialogs, published list, render log use the fill
      rule; fixed widths in window code become proportional or listed (t1127)
- [x] 1.4 `window_fill_tests.rs`: t1125 (corner drag, body fills, modals listed), t1126 (sizes
      remembered), t1127 (fixed-size scan with reasons); t779, t918, t923, t921, t757 stay green

## 2. The Guide

- [x] 2.1 `guide.rs`: grid tables with measured columns, header fill, rules, stripes (D4) — t1128
- [x] 2.2 index cards and search-heading cards; proportional sides; both lists fill (D5) — t1129, t1130;
      t989 stays green

## 3. The effect picker

- [x] 3.1 `fx_registry.rs`: one scroll, width from the host window, height from the screen, presets
      as a cell row (D6) — t1131; t547, t1018 and the picker tests stay green
- [x] 3.2 the media picker list height from the screen

## 4. The toolbar

- [x] 4.1 `toolbar.rs`: groups, measured widths, `⋯` overflow menu with the same bodies (D7) — t1132;
      t1094 and the toolbar tests stay green

## 5. Live and docs

- [x] 5.1 live scenario `src/scripts/live/window_fill.py`: the effects window, the Guide and the picker
      at a small and a large size, the toolbar at 1100 px — a screenshot per step
- [x] 5.2 `docs/guide/whats-new.md` (kt26.11) line; registry K12 → done; `docs/user-feedback.md` taken
      as; `src/apps/kt-app/CLAUDE.md` window rule; `ktprompts-index.py --check`, `specs-user-path.py --check`
- [x] 5.3 `cargo test --workspace`, `cargo clippy --all-targets`; `git merge master`;
      `src/scripts/finish.sh` — the version here and in the report
      — `cargo test --workspace` green (finish.sh: 1340 passed, 14 skipped), clippy: no new warnings;
      `git merge master` already up to date; finish.sh **kt26.11.01-ae184f46**, live scenarios green
      (the parallel pass lost its X displays under load; every red passed alone on the second try,
      `window_fill` included)
