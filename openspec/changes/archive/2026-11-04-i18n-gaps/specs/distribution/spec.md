# Spec Delta

## ADDED Requirements

### Requirement: Desktop integration speaks every interface language and claims only our files
The desktop entry (`src/packaging/kinetren.desktop`) and the MIME description
(`src/packaging/kinetren-mime.xml`) SHALL carry a translation for EVERY language of the
interface catalogs (`src/apps/kt-app/i18n/*.toml`), mapped to the freedesktop locale names
(`ru` → `ru`, `es` → `es`, `ar` → `ar`, `zh-Hans` → `zh_CN`; English is the untranslated value):
`GenericName`, `Comment` and `Keywords` of the desktop entry, and the `<comment>` of every MIME
type. The translated texts SHALL come from one place — a section of the catalogs — and the
packaging files SHALL be equal to it; a new catalog language without its packaging lines is red.
The desktop entry SHALL have `Keywords=` (English and translated) naming what a person types in
an application menu to find a video editor.
The MIME description SHALL claim only files that are ours: the project type
`application/x-kinetren-project` SHALL be recognised by the glob `*.kt` TOGETHER with a content
match on the project file's root marker (`"kinetren"` near the start of the file), because
`*.kt` is also the Kotlin source glob of shared-mime-info (`text/x-kotlin`) and a glob alone would
make every Kotlin source a "Kinetren project". The legacy `.ktproj` SHALL NOT be described as a
Kinetren project: it gets its own type ("Older Kinotrain project") so a file manager names it
truthfully, and opening it with Kinetren reaches the "Open" refusal that names the import door
(ui-reach, "Import of an old-version project").
Why: kkt item 3 (2026-10-27): "`kinetren-mime.xml` is not fully translated" — the desktop and
MIME files carried only `ru` next to English while the program speaks five languages; the MIME
file still globbed `*.ktproj` as a Kinetren project although R.70 made `.kt` the project file and
`.ktproj` an import-only legacy format (project-format design: "two formats under one suffix is a
trap").

#### Scenario: Every catalog language is in the desktop and MIME files
- **WHEN** the catalog languages are listed from the build tables and both packaging files are parsed; each translated value is compared with the catalogs' packaging section; separately a copy of the tree gets a sixth catalog language, and a copy has `Comment[es]` removed
- **THEN** for every language and every key (`GenericName`, `Comment`, `Keywords`, each MIME `<comment>`) the packaging value equals the catalog value (the keyword list joined with `;` and ending with `;`); `Keywords=` exists untranslated; both broken copies are red naming the language and the key
- **TEST** t930 (`src/apps/kt-app/src/i18n_tests.rs`)

#### Scenario: The MIME file claims only our files
- **WHEN** `kinetren-mime.xml` is parsed by the distribution guard
- **THEN** `application/x-kinetren-project` has the glob `*.kt` and a magic rule matching the root marker `"kinetren"` within the first bytes; no type of ours globs `*.ktproj` except the legacy type, whose comment names the old program; `application/x-opentimelineio` globs `*.otio`; the desktop `MimeType=` lists exactly the three types; a copy with `*.ktproj` back on the project type is red
- **TEST** t931 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The desktop database resolves the files the way the person expects
- **WHEN** in a temporary `XDG_DATA_HOME` the MIME file is installed and `update-mime-database` is run, and the type of three files is queried: a project written by `kt_model::project_file::to_text` named `film.kt`, a Kotlin source `Main.kt`, and the legacy `fixtures/demo.ktproj`
- **THEN** `film.kt` is `application/x-kinetren-project`, `Main.kt` is `text/x-kotlin`, `demo.ktproj` is the legacy type; without `update-mime-database` on the machine the test prints why it did not run, and the build container installs it so CI always runs it
- **TEST** t935 (`src/apps/kt-cli/tests/dist_guard.rs`)
