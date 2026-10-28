# Proposal

## Why

Two complaints of the 2026-10-27 round (`docs/user-feedback.md`, "The owner's list of 2026-10-27
and ktprompts v26.10.18"): kkt item 3 — "`kinetren-mime.xml` is not fully translated", and
ktprompts 26.10.18 item 10 — "in a composition's fx-chain there is a published element with
parameters, and in the English version it literally says «Лента»". Both are languages the
program speaks that leak past the catalogs: the desktop and MIME files carry only `ru` next to
English (`src/packaging/kinetren.desktop`, `src/packaging/kinetren-mime.xml`) while the interface
has five catalogs (en, ru, es, ar, zh-Hans); and «Лента» is not a hard-coded string (t131 keeps
Cyrillic out of literals and is green) but the default reel name that the collapse gesture writes
into the project file in the interface language of that moment (`tr(k::msg::NEST_DEFAULT_NAME)`,
`src/apps/kt-app/src/app.rs:3118`, `:3256`), without asking. Neither has a guard, so the next
language or the next default name drifts the same way.

## What Changes

- **Packaging translations from the catalogs.** A `[desktop]` section in every catalog holds the
  desktop entry's generic name, comment and keywords and the comments of the MIME types; the
  desktop and MIME files carry all of them (`es`, `ar`, `zh_CN` added next to `ru`); a guard keeps
  the files equal to the catalogs for every catalog language.
- **`Keywords=`** in the desktop entry, English and translated.
- **The MIME file claims only our files.** The project type keeps `*.kt` but adds a content match
  on the root marker `"kinetren"` (`*.kt` is also Kotlin's glob); `*.ktproj` leaves the project
  type and gets its own "Older Kinotrain project" type, in line with R.70 (`.kt` is the project
  file, `.ktproj` is import-only). A real `update-mime-database` run checks the three outcomes.
- **A reel name the program chose follows the interface language.** A reel whose name is the
  default reel name of any catalog (optionally with a number) is SHOWN in the current language in
  every place that shows reel names — the owner's case is the head of the instance's chain strip.
  The stored name is not rewritten; a name the person typed is shown as typed.
- **Cyrillic guard beyond literals.** Cyrillic is allowed only in `ru.toml`, the endonym `Русский`,
  and declared test data; a non-Russian catalog or a committed project fixture with Cyrillic is red.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `distribution`: the desktop entry and the MIME description carry every catalog language from one
  catalog section, have `Keywords=`, and claim only our files (`*.kt` with a content match, the
  legacy `.ktproj` as its own type).
- `ui-timeline`: the requirement "Localization — five catalogues, not one label missed" gains the
  Cyrillic-placement rule and its guard; a new requirement makes a program-chosen reel name follow
  the interface language.

## Impact

- Packaging: `src/packaging/kinetren.desktop`, `src/packaging/kinetren-mime.xml`;
  `src/packaging/install.sh` and `release-build.sh` unchanged (they copy the files as they are);
  `src/packaging/Dockerfile` installs `shared-mime-info` for t935 if the image lacks it.
- Catalogs: `src/apps/kt-app/i18n/{en,ru,es,ar,zh-Hans}.toml` (new `[desktop]` section; exempt from
  t125's "used by a Rust path" check like `[lang]`, used by the packaging guard instead);
  `python3 scripts/subset-fonts.py` only if a new CJK/Arabic glyph appears (the section is not
  drawn by egui, so t100 does not need it; the script reads all catalogs anyway).
- Code (kt-app): one display function for reel names and its callers (the strip head in
  `widgets.rs` via `fx_editor.rs`, crumbs `app.rs:nest_crumbs`, bin card `bin_panel.rs`, the
  settings window title `settings_window.rs:129`, search hits, the rename dialog prefill
  `timeline.rs:5903`, `bin_panel.rs:1147`).
- Tests: `i18n_tests.rs` (t930, t933), `layout_tests.rs` (t932, t934), `src/apps/kt-cli/tests/dist_guard.rs`
  + `src/scripts/dist-check.py` (t931, t935).
- No project-format change (the shown name is computed, nothing is stored); no engine change.
