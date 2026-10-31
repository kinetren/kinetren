## ADDED Requirements

### Requirement: Each live guard runs in its own output folder
The guards of the live framework that run `scripts/live-check.sh` (t801, t802) SHALL each pass
their own `KT_LIVE_OUT` (`target/live-guard-<test>`), never a shared one. Why: `live-check.sh`
deletes `verdicts/` of its output folder at its start, and the tests run in parallel. On CI of
94e36e9e t802 started while t801 was re-running its red scenario and wiped the verdict:
"the re-run scenario stayed red", with an empty verdict.

#### Scenario: Two guards run at once and keep their verdicts
- **WHEN** t801 (a re-run of a red scenario) and t802 (three isolated workers) run in parallel
- **THEN** t801 reads its re-run verdict `ok` from `target/live-guard-t801/verdicts`, and t802
  reads its workers' `env.json` from `target/live-guard-t802`
- **TEST** t801, t802 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: A canonical path is shown as a person reads it
A path made canonical for display or as a key next to `std::path::absolute` spellings SHALL go
through `kt_dirs::canonical`, never the bare `std::fs::canonicalize`: on Windows the verbatim
prefix is dropped where the plain spelling names the same file (`\\?\C:\x` → `C:\x`,
`\\?\UNC\h\s\x` → `\\h\s\x`) and kept where it would not (past `MAX_PATH`, not a drive or UNC
path). The rule is `kt_dirs::plain_path`, pure over `Os`. `user_store::project_key` — the key of
a Recent row and its poster, and the path its welcome card shows — uses it. Why: on windows-latest
t986 failed "each card names its folder": the card named `\\?\C:\…`, a folder nobody typed.

#### Scenario: The verbatim prefix is dropped where it means nothing
- **WHEN** `plain_path` reads a verbatim drive path, a verbatim UNC path, a plain path, a
  `\\?\Volume{…}` path, a verbatim path past `MAX_PATH`, and a Linux path spelled the same; and
  `canonical` resolves an existing folder
- **THEN** the first two lose the prefix, the rest come back unchanged, and `canonical` gives an
  absolute path without the prefix that canonicalises to the same file
- **TEST** t1062 (`src/libs/kt-dirs/tests/dirs.rs`)

#### Scenario: A recent card names the folder without the prefix
- **WHEN** the welcome window shows two recent projects of one folder
- **THEN** both cards name the folder in its canonical spelling, and no label carries `\\?\`
- **TEST** t986 (`src/apps/kt-app/src/reach_tests.rs`)
