# Spec Delta

## MODIFIED Requirements

### Requirement: Localization — five catalogues, not one label missed
The source language of literals SHALL be English; UI text SHALL live only in the
catalogs `i18n/{en,ru,es,zh-Hans,ar}.toml` and be taken via `tr`/`tf!` with
positional placeholders; there is no plural form ("noun: N").
Catalogs SHALL be complete, with no extra keys and no unused ones. Every
non-ASCII character of string literals and catalogs SHALL have a glyph in the six
loadable fonts; icons come from Phosphor (PUA) or as vectors; ideographs and
Arabic come from Noto subsets assembled by a script. The font order MUST NOT
change. Arabic is reordered WORD BY WORD (shaping atoms); the content of a
`TextEdit` — never. Every window SHALL have an explicit id. Strings in a background
thread MUST NOT be translated. The language on first launch comes from the OS locale; the choice
is stored outside the project. Layout is checked with the real fonts in all languages.
Cyrillic (U+0400…U+052F) SHALL appear only in `i18n/ru.toml`, in the endonym `Русский` of the
`[lang]` section of every catalog, and in test data declared as such; a non-Russian catalog and
a committed project fixture MUST NOT carry it.
Why: the user twice got a square placeholder stub; changing a window title
would reset its position; per-character reversal makes an Arabic phrase worse.
2026-10-27 (ktprompts 26.10.18 item 10): "in a composition's fx-chain there is a published
element with parameters, and in the English version it literally says «Лента»" — t131 already
kept Cyrillic out of string literals, so the word came from somewhere a literal guard does not
look: a name the program itself stored in the project.

#### Scenario: Glyphs of all non-ASCII characters
- **WHEN** the kt-app sources (recursively) and the `i18n/*.toml` catalogs are scanned, expanding `\u{…}`
- **THEN** every non-ASCII character has a glyph; test code is cut off at `#[cfg(test)]` with zero indentation
- **TEST** t100 (`src/apps/kt-app/src/font_tests.rs`)

#### Scenario: CJK and Arabic subsets are rebuilt from the catalogs
- **WHEN** a new ideograph or an Arabic ligature is added to a `i18n/*.toml` catalog
- **THEN** the Noto subsets are rebuilt by the script from the catalog contents (`python3 scripts/subset-fonts.py`), otherwise t100 turns red on the new glyph; the full font is not put into the binary
- **TEST** t100 (`src/apps/kt-app/src/font_tests.rs`) after `src/scripts/subset-fonts.py`

#### Scenario: Catalogs are complete and used
- **WHEN** the build generates tables from the catalogs
- **THEN** a missing/extra key, a mismatch of placeholders, PUA in a catalog, an ASCII comma in Arabic break the build; a key that nobody calls by its FULL path is red; the parser is strict
- **TEST** t125 (`src/apps/kt-app/src/i18n_tests.rs`)

#### Scenario: Arabic — the order of words, not of letters
- **WHEN** a phrase of two Arabic words is rendered; the search sections are read
- **THEN** the leftmost glyph of the word is the last logical letter, the second word is to the left of the first; the sections `[fxreg.kw]`/`[lang]` are not reordered
- **TEST** t126 (`src/apps/kt-app/src/i18n_tests.rs`), t126 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Language switch on the fly and locale detection
- **WHEN** three windows are open, the language is switched; first launch with `LC_ALL`/`LANG`
- **THEN** the window positions are kept (explicit id); `zh_TW`/`zh_HK` → simplified, `C`/`POSIX` → English; the user's choice overrides
- **TEST** t129 (`src/apps/kt-app/src/layout_tests.rs`), t130 (`src/apps/kt-app/src/i18n_tests.rs`)

#### Scenario: Literals in English
- **WHEN** `src/**/src/**/*.rs` of the whole workspace is walked
- **THEN** zero Cyrillic in string and character literals outside test code
- **TEST** t131 (`src/apps/kt-app/src/i18n_tests.rs`)

#### Scenario: Cyrillic only where it is the subject
- **WHEN** the catalogs `en`, `es`, `zh-Hans`, `ar` are scanned value by value, and the committed project and exchange fixtures (`src/fixtures/**` `.kt`/`.otio`/`.ktproj`, the live-scenario projects) are scanned as text; a Cyrillic word is temporarily put into `es.toml` and into a copy of a fixture
- **THEN** the only Cyrillic found in the four catalogs is the endonym `Русский` of `[lang]`; a fixture carries Cyrillic only if it is in the test's declared list of Cyrillic test data, each with a reason; both temporary additions are red with the file and the key or line
- **PATH** there is no gesture: the guard reads the catalogs and fixtures and names the file and the key or line of a Cyrillic word that does not belong there — it is verified by running the test, not by mouse
- **TEST** t933 (`src/apps/kt-app/src/i18n_tests.rs`)

## ADDED Requirements

### Requirement: A name the program chose follows the interface language
A reel that the program named by itself — the default reel name of the collapse
gesture, with or without the number `unique_name` adds — SHALL be SHOWN in the current
interface language wherever a reel name is shown (the head of an instance's chain strip, the path
crumbs, the media bin card, the reel's settings window title, search hits, the rename dialog's
prefill). A name is "chosen by the program" when it equals the default reel name of ANY of the
five catalogs, optionally followed by a space and a number; any other name is the person's text
and is shown exactly as stored. The stored name in the project file SHALL NOT change by being
shown; renaming stores what the person typed. Lookups by name (search, the rename uniqueness check)
SHALL compare both the stored and the shown form.
Why: ktprompts 26.10.18 item 10. The default name is written into the file silently at collapse
(`tr(msg.nest_default_name)`, no question asked), so a reel created in the Russian interface
carried «Лента» into every other language — a word the person never typed. Existing files are
covered too, because the rule reads the name, not a flag.

#### Scenario: A default reel name is shown in the current language
- **WHEN** a selection is collapsed twice in the Russian interface (names «Лента», «Лента 2»), a third reel is renamed «Интро», the project is saved and reopened in English; then switched to Chinese
- **THEN** in English the reels are shown as "Reel", "Reel 2" and «Интро»; in Chinese as the zh-Hans default name, its "2", and «Интро»; the project file still holds «Лента», «Лента 2», «Интро» byte for byte; renaming "Reel 2" to "Outro" stores "Outro"
- **PATH** in the Russian interface select two events → Collapse into a reel → the bin card «Лента» → View → Language → English → the bin card reads "Reel", the instance on the timeline reads "Reel" → right-click it → Rename the reel… → the field shows "Reel" → type "Outro" → Enter → the card reads "Outro" in every language
- **TEST** t932 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Every place that shows a reel name uses the shown form
- **WHEN** a reel stored as «Лента» has an instance with one published parameter; in English the instance's FX window is opened, the reel is entered (crumbs), its settings window is opened, the bin is shown and Ctrl+F searches "Reel"
- **THEN** the head of the instance's chain strip reads "📌 Reel" (the owner's case), the crumbs, the settings window title, the bin card and the search hit all read "Reel"; the search finds the reel by "Reel" and by «Лента»
- **PATH** double-click the reel instance → the FX window → the head of the strip "📌 Reel", the published parameter below it → Enter the reel → crumbs "Root › Reel" → Ctrl+F, type "Reel" → the reel is found
- **TEST** t934 (`src/apps/kt-app/src/layout_tests.rs`)
