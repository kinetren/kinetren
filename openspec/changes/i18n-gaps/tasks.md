# Tasks

Test numbers t930–t935 are reserved by this change. Every stage ends with a commit (English
message, first line = the stage and what was done). Stage only this change's files.

## 0. Records before code

- [x] 0.1 The owner's kkt item 3 and ktprompts 26.10.18 item 10 are recorded in `docs/user-feedback.md` (round "The owner's list of 2026-10-27 and ktprompts v26.10.18") and name `i18n-gaps`. Verify `openspec validate i18n-gaps --strict` is green

## 1. Prove the source of «Лента», tests first

- [ ] 1.1 Write t932 in `src/apps/kt-app/src/layout_tests.rs` on our fixture: collapse twice in the Russian interface, rename a third reel, save through `project_file::to_text`, reopen in English and in Chinese. Verify it is RED on today's code exactly by showing «Лента» in English (this proves the source is the stored default name, design "Context"), and that the saved text holds «Лента» — record the failure line here
- [ ] 1.2 Write t934 in `layout_tests.rs`: the head of the instance's chain strip (the owner's place), crumbs, bin card, the settings window title, the search hit. Verify RED and list which places show «Лента»
- [ ] 1.3 Write t933 in `src/apps/kt-app/src/i18n_tests.rs` (non-Russian catalogs value by value, committed fixtures as text, the declared Cyrillic test-data list with reasons). Verify it is green on today's tree (no hidden Cyrillic) and red on the two temporary additions of the spec, and that t799 accepts the PATH lines of t932–t934

## 2. Desktop and MIME files

- [ ] 2.1 Add the `[desktop]` section (`generic_name`, `comment`, `keywords` space-separated, `mime_project`, `mime_otio`, `mime_legacy`) to all five catalogs; add `desktop.` to `NOT_REFERENCED_BY_PATH` in `i18n_tests.rs`. Verify `cargo build -p kt-app` (the catalog build rules: no ASCII `;`/`,` in Arabic, ≤160 characters) and t125 green
- [ ] 2.2 Write t930 in `i18n_tests.rs`; verify it is RED (es, ar, zh_CN missing, no `Keywords`); then edit `src/packaging/kinetren.desktop` (`GenericName`/`Comment`/`Keywords` for en, ru, es, ar, zh_CN) and `src/packaging/kinetren-mime.xml` (comments for every language). Verify t930 green
- [ ] 2.3 MIME structure (design D2): the magic rule on `"kinetren"` for the project type, the legacy type `application/x-kinotrain-legacy-project` for `*.ktproj`, `MimeType=` with the three types; extend `src/scripts/dist-check.py` and `dist_guard.rs` (`FILES` gains the MIME file) with t931. Verify t931, t834, t835 green and the broken copy of the spec is red
- [ ] 2.4 Write t935 in `src/apps/kt-cli/tests/dist_guard.rs` (temporary `XDG_DATA_HOME`, `update-mime-database`, query the type of `film.kt` written by `to_text`, a Kotlin `Main.kt`, `fixtures/demo.ktproj`); make sure `shared-mime-info` is in `src/packaging/Dockerfile` and in the environment where `.github/workflows/ci.yml` runs `cargo test`. Verify t935 green locally and record whether CI runs it; t860 (workflow shape) still green
- [ ] 2.5 Commit stages 1–2 (`cargo test --workspace --no-fail-fast` green)

## 3. Reel names in the current language

- [ ] 3.1 Add the display function of design D3 next to the catalog access in kt-app (the five default names from the build tables, the optional `" N"` suffix). Verify a unit test inside t932's module for the matching rules (exact word, with number, with a different word) is green
- [ ] 3.2 Route every place that shows a reel name through it (strip head via `fx_editor.rs`/`widgets.rs`, `nest_crumbs` in `app.rs`, `bin_panel.rs`, `settings_window.rs:129`, search hits, the rename dialog prefill at `timeline.rs:5903` and `bin_panel.rs:1147`); the search and the rename uniqueness check compare both forms. Verify t932, t934 green, t793 (strip head width) and t129 (language switch keeps windows) still green
- [ ] 3.3 Run `font_tests.rs` and `i18n_tests.rs` whole (t100, t125, t126, t128, t131), `reach_tests.rs` and `layout_tests.rs`. Verify green, then commit stage 3

## 4. Live and records

- [ ] 4.1 Live scenario `src/scripts/live/nestparam.py` (extend it): with `KT_LANG=ru` collapse a selection, save, restart with English, open the instance's FX window — `kt_live.step` screenshots of the strip head "📌 Reel" and the bin card. Verify `./scripts/live-check.sh nestparam` green, screenshots listed here
- [ ] 4.2 Manual check recorded here: `make install` on this machine, then the file manager shows `film.kt` as "Kinetren project" (in Spanish with `LANGUAGE=es`), a Kotlin file as Kotlin source, `demo.ktproj` as "Older Kinotrain project"; the application menu finds Kinetren by "montage"
- [ ] 4.3 Commit stage 4

## 5. Finish

- [ ] 5.1 Run `./scripts/finish.sh` from `src/`. Verify it is green and record the version `vYY.MM.DD-hash` and the live screenshots here
- [ ] 5.2 After archiving: append to `openspec/specs/distribution/design.md` the "why" of D1 (catalog section, no generator) and D2 (Kotlin glob, the legacy type), and to `openspec/specs/ui-timeline/design.md` the "why" of D3 (recognised by text, no format flag) and D4
