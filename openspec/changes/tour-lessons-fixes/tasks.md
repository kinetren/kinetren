# Tasks

## 1. The step model and the re-check (K2, K3)

- [x] 1.1 `lessons.rs`: `Step { text: &[Key], args, targets, done, result, look }`; `Observed` becomes a
      `Copy` key (model_rev, region, loop, window bits, render result); `open_window_ids` without a `Vec`
      — t1085
- [x] 1.2 Lesson text: `guide::inlines` per line, `Arg::Label`/`Arg::Path` substitution, the Arabic
      logical parse + per-run reorder, the memo per (language, lesson, step); the panel draws lines,
      list items and keycaps through the Guide's run renderer — t1086
- [x] 1.3 Catalogs ×5: every step rewritten as lines with `<kbd>` and bold; lesson 3 teaches "Transition
      with previous"; result lines; eight jokes; `scripts/subset-fonts.py`; font/i18n tests green — t1086,
      t100, `i18n_tests`

## 2. Targets, outlines and "Show" (K4, K5, K6)

- [x] 2.1 `widgets::lesson_mark` (current targets, the outline, the recorded rects); marks in
      `gen_ui::generator_menu`, `fx_registry` (+ effect…), `fx_editor` (+ curve), `toolbar` (Loop,
      Render, the File render item), `timeline` (Transition with previous), `transition_editor` (kind),
      `render_dialog` (Start); the closed `CONTROLS` list and its source guard — t1087, t1092
- [x] 2.2 Timeline targets from `tl_geom`: event, marker, ruler, transition, morpher block, clipped to the
      tracks — t1087
- [x] 2.3 "Show": menu roots by key, centre the view and the row, `scroll_to_me` for a control, the
      fallback to the next target; nothing opens a step's window — t1088
- [x] 2.4 Lesson 3: the overlap does not close the step, the right-click item does and is outlined —
      t1089
- [x] 2.5 t988 and `no_teaching_layer_over_the_ui` follow the new step shape (no weakening)

## 3. Result lines, the playhead, jokes (K7, K8)

- [x] 3.1 `Step::result` shown under the checked step; `Step::look` seeks once when the step closes
      (lessons 2, 3, 4) — t1090
- [x] 3.2 The render jokes, rotation on the UI clock with one repaint per switch — t1091

## 4. A person's walk and the records

- [x] 4.1 Live scenario `src/scripts/live/lessons.py`: all five lessons through the real gestures, a
      screenshot per step; anything else of the same kind found is fixed and recorded as a scenario.
      Found and fixed: a click on a marker did nothing (t1093); the panel clipped the toolbar's Loop
      button (t1094, D8); "Next lesson" asked to save the sandbox (t1095)
- [x] 4.2 `docs/guide/whats-new.md` (kt26.11), the registry rows K2–K8 (`kkt-26.11.01-index.md`),
      F-HLP-028's verdict addendum (the jokes), `ktprompts-index.py --check`, `features-index.py --check`
- [x] 4.3 `cargo test -p kt-app`, `cargo test --workspace`, `cargo clippy --all-targets`
      (clippy clean for the change; t640 fails on a quiet machine with "sqlite: database is
      locked" with or without this change — the reel sidecar tier, not touched here, follow-up)
- [x] 4.4 `src/scripts/finish.sh` — the version here and in the report; archive the change.
      Version **kt26.11.01-79bf50ab** (after the master merge); live: 57 scenarios, `lessons` green,
      `loop` red twice under load and green alone on a quiet machine
