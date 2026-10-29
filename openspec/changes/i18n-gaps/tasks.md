# Tasks

Test numbers t930–t935 are reserved by this change. Every stage ends with a commit (English
message, first line = the stage and what was done). Stage only this change's files.

## 0. Records before code

- [x] 0.1 The owner's kkt item 3 and ktprompts 26.10.18 item 10 are recorded in `docs/user-feedback.md` (round "The owner's list of 2026-10-27 and ktprompts v26.10.18") and name `i18n-gaps`. Verify `openspec validate i18n-gaps --strict` is green

## 1. Prove the source of «Лента», tests first

- [x] 1.1 Write t932 in `src/apps/kt-app/src/layout_tests.rs` on our fixture: collapse twice in the Russian interface, rename a third reel, save through `project_file::to_text`, reopen in English and in Chinese. Verify it is RED on today's code exactly by showing «Лента» in English (this proves the source is the stored default name, design "Context"), and that the saved text holds «Лента» — record the failure line here
  - t932 (`layout_tests.rs`, our three-clip scene): three collapses in the Russian interface store
    «Лента», «Лента 2», «Лента 3»; the third renamed «Интро»; `project_text_of` holds «Лента»,
    «Лента 2», «Интро» — so the source of the owner's «Лента» is the stored default name (design
    "Context" confirmed on our fixture). RED on today's display (the identity, which is what the code
    did): "En: no “Reel” shown" — the bin card read «Лента». The bell and the status line keep the
    Russian SENTENCE of the collapse ("Свёрнуто в ленту: Лента 3"): history in the language of its
    moment, cleared before the switch, not a shown reel name. t932b holds the matching rules.
- [x] 1.2 Write t934 in `layout_tests.rs`: the head of the instance's chain strip (the owner's place), crumbs, bin card, the settings window title, the search hit. Verify RED and list which places show «Лента»
  - t934 (`layout_tests.rs`, the news template — a reel collapsed in Russian with published
    parameters): in English the strip head, the bin card, the search (by "Reel" and by «Лента»), the
    crumbs and the reel settings title. RED on today's display at the owner's place first: "the strip
    head is not “📌 Reel”: ["📌6", "📌 Лента"]". The places that showed the stored name: the strip
    head (`fx_editor.rs`), the bin card and its rename prefill (`bin_panel.rs`), the timeline's rename
    prefill and an expanded band's row labels (`timeline.rs`), the crumbs, the settings title, the two
    published-parameter windows (`pubparam.rs`), the "made independent" note; search did not find a
    reel instance at all (its clip has no media name).
- [x] 1.3 Write t933 in `src/apps/kt-app/src/i18n_tests.rs` (non-Russian catalogs value by value, committed fixtures as text, the declared Cyrillic test-data list with reasons). Verify it is green on today's tree (no hidden Cyrillic) and red on the two temporary additions of the spec, and that t799 accepts the PATH lines of t932–t934
  - t933 (`i18n_tests.rs`): the four other catalogs value by value (only `[lang] ru`), and the
    fixtures, the live scenarios and every integration `tests/` of the workspace as text against
    `CYRILLIC_TEST_DATA` (three entries with reasons: the title golden, the non-ASCII path round trip,
    the README guard's injected line). It asserts the walk reached the declared files (a walk that
    misses `tests/` would pass empty) and proves both scanners on broken copies in memory (a Russian
    value in `es.toml`, a Cyrillic fixture line) — the tree is never written. Green; t799 green.

## 2. Desktop and MIME files

- [x] 2.1 Add the `[desktop]` section (`generic_name`, `comment`, `keywords` space-separated, `mime_project`, `mime_otio`, `mime_legacy`) to all five catalogs; add `desktop.` to `NOT_REFERENCED_BY_PATH` in `i18n_tests.rs`. Verify `cargo build -p kt-app` (the catalog build rules: no ASCII `;`/`,` in Arabic, ≤160 characters) and t125 green
  - `[desktop]` (6 keys) in all five catalogs; Arabic keywords space-separated, no ASCII `;`/`,`;
    `desktop.` exempt in t125. `subset-fonts.py` rerun (no new glyphs needed). t125, t100, t126, t128
    green.
- [x] 2.2 Write t930 in `i18n_tests.rs`; verify it is RED (es, ar, zh_CN missing, no `Keywords`); then edit `src/packaging/kinetren.desktop` (`GenericName`/`Comment`/`Keywords` for en, ru, es, ar, zh_CN) and `src/packaging/kinetren-mime.xml` (comments for every language). Verify t930 green
  - t930 RED first (es, zh_CN, ar missing, no `Keywords`, no legacy comments), then
    `kinetren.desktop` and `kinetren-mime.xml` written for all five languages; green. It proves itself
    on a sixth language and on a removed `Comment[es]` in memory. `desktop-file-validate` passes.
- [x] 2.3 MIME structure (design D2): the magic rule on `"kinetren"` for the project type, the legacy type `application/x-kinotrain-legacy-project` for `*.ktproj`, `MimeType=` with the three types; extend `src/scripts/dist-check.py` and `dist_guard.rs` (`FILES` gains the MIME file) with t931. Verify t931, t834, t835 green and the broken copy of the spec is red
  - MIME: the project type `*.kt` + magic `"kinetren"` at 0:64; `application/x-kinotrain-legacy-project`
    for `*.ktproj`; `MimeType=` with the three types. `dist-check.py::check_mime` + t931 (four broken
    copies: no magic, `*.ktproj` in the project type, no legacy type, a short `MimeType=`); t834, t835
    green. `install.sh` already runs `update-mime-database` and `update-desktop-database`. The name
    guard t833 was taught the legacy type (its id, `mime_legacy`, the lines inside its MIME block) — it
    names the old program by design.
- [x] 2.4 Write t935 in `src/apps/kt-cli/tests/dist_guard.rs` (temporary `XDG_DATA_HOME`, `update-mime-database`, query the type of `film.kt` written by `to_text`, a Kotlin `Main.kt`, `fixtures/demo.ktproj`); make sure `shared-mime-info` is in `src/packaging/Dockerfile` and in the environment where `.github/workflows/ci.yml` runs `cargo test`. Verify t935 green locally and record whether CI runs it; t860 (workflow shape) still green
  - t935 (`dist_guard.rs`): a temporary `XDG_DATA_HOME` with our file plus the system's
    `freedesktop.org.xml`, `update-mime-database`, `xdg-mime query filetype`: `film.kt` (written by
    `to_text`) → ours, `Main.kt` → `text/x-kotlin`, `demo.ktproj` → the legacy type. Green locally;
    RED without the magic rule: our own `film.kt` became "text/x-kotlin". `shared-mime-info` and
    `xdg-utils` added to the container's base stage and to the CI `test` job's apt list (that job runs
    the tests on the runner, under nextest), so CI runs it; t860 green.
- [x] 2.5 Commit stages 1–2 (`cargo test --workspace --no-fail-fast` green)
  - Committed together with stage 3 (one commit): t804 went red on the new test strings — the
    coverage collector skipped only two of the `*_tests.rs`; it now skips all of them and the
    ratchet `UNTOUCHED_LIMIT` goes 84 → 81. kt-app 403, kt-cli green.

## 3. Reel names in the current language

- [x] 3.1 Add the display function of design D3 next to the catalog access in kt-app (the five default names from the build tables, the optional `" N"` suffix). Verify a unit test inside t932's module for the matching rules (exact word, with number, with a different word) is green
  - `i18n::reel_display_name(stored) -> Cow<str>`: the default of any catalog (both Arabic forms),
    alone or + `" " + digits` → the current default + the same number; t932b: exact word, number,
    other words, the person's text ("Лента новостей", "Reel2", "My Reel") untouched.
- [x] 3.2 Route every place that shows a reel name through it (strip head via `fx_editor.rs`/`widgets.rs`, `nest_crumbs` in `app.rs`, `bin_panel.rs`, `settings_window.rs:129`, search hits, the rename dialog prefill at `timeline.rs:5903` and `bin_panel.rs:1147`); the search and the rename uniqueness check compare both forms. Verify t932, t934 green, t793 (strip head width) and t129 (language switch keeps windows) still green
  - Routed: strip head, bin card and rename prefill, timeline rename prefill, band row labels,
    crumbs, settings title, both published-parameter windows, the "independent" note; search gives a
    reel instance its reel's shown name and matches the stored one too; the rename dialog's taken list
    holds both forms. t932, t934, t793, t129 green.
- [x] 3.3 Run `font_tests.rs` and `i18n_tests.rs` whole (t100, t125, t126, t128, t131), `reach_tests.rs` and `layout_tests.rs`. Verify green, then commit stage 3
  - font_tests, i18n_tests, reach_tests, layout_tests: kt-app 403 passed.

## 4. Live and records

- [x] 4.1 Live scenario `src/scripts/live/nestparam.py` (extend it): with `KT_LANG=ru` collapse a selection, save, restart with English, open the instance's FX window — `kt_live.step` screenshots of the strip head "📌 Reel" and the bin card. Verify `./scripts/live-check.sh nestparam` green, screenshots listed here
  - A scenario of its own, `src/scripts/live/nestlang.py` (`OWN_PROCESS = True`), not part of
    `nestparam.py`: the language is read at start-up, and two own processes inside the stand's run put
    a second window over the stand's (the first try clicked the stand's window). Russian: collapse two
    events, save — the file holds «Лента»; English on the same file: the bin card, "Composition
    parameters" → the strip head "📌 Reel" (`fx.head` probe, screenshot), entering → the dump's crumb
    `nest.name` is "Reel"; the file still holds «Лента». Green. Screenshots:
    `target/live-out/nestlang/01-step-ru-collapsed.png`, `01-step-en-bin.png`, `02-step-en-strip-head.png`,
    `03-step-en-crumbs.png`.
- [ ] 4.2 Manual check recorded here: `make install` on this machine, then the file manager shows `film.kt` as "Kinetren project" (in Spanish with `LANGUAGE=es`), a Kotlin file as Kotlin source, `demo.ktproj` as "Older Kinotrain project"; the application menu finds Kinetren by "montage"
  - OPEN — the owner's: `make install` would put a development build over the installed Kinetren in
    the home directory, so it is not run by the agent. What it would show is held by t935 against a
    temporary MIME database (our `film.kt`, a Kotlin `Main.kt`, `demo.ktproj`), and the desktop file
    passes `desktop-file-validate`. To do by hand: `make install`, then the file manager on `film.kt`,
    `Main.kt`, `demo.ktproj` (with `LANGUAGE=es` too) and the application menu search "montage".
- [x] 4.3 Commit stage 4
  - Committed.

## 5. Finish

- [ ] 5.1 Run `./scripts/finish.sh` from `src/`. Verify it is green and record the version `vYY.MM.DD-hash` and the live screenshots here
- [ ] 5.2 After archiving: append to `openspec/specs/distribution/design.md` the "why" of D1 (catalog section, no generator) and D2 (Kotlin glob, the legacy type), and to `openspec/specs/ui-timeline/design.md` the "why" of D3 (recognised by text, no format flag) and D4
