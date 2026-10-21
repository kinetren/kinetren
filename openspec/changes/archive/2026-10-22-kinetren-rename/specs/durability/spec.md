# Spec Delta

## ADDED Requirements

### Requirement: Per-user data lives in `kinetren`; the former directory is moved once

The application's per-user data directory SHALL be `$XDG_DATA_HOME/kinetren` (falling
back to `~/.local/share/kinetren`). This covers recovery files, the journal and crash
report, project locks, presets, the workspace and UI state. If the directory does not
exist and the former `kinotrain` directory does, the first start SHALL move the former
one into place before anything is read or written. This happens once, and nothing is
lost or duplicated. If the move fails (for example, across filesystems), the content
SHALL be copied and the former directory left in place. If both directories exist, the
new one wins and the former one is not touched. `kt-cli` SHALL look for project locks in
the same directory as the app.

Why: the full rename must not lose the user's presets, workspace, recovery copies or
locks.

#### Scenario: First start moves the former directory; later starts do not touch it
- **WHEN** the data directory is resolved with only `kinotrain/` present (holding a preset file and a lock); then again; then with both present
- **THEN** the first call leaves `kinetren/` with the same files and no `kinotrain/`; the second changes nothing; with both present the new one is used and the former is untouched
- **TEST** t830 (`src/apps/kt-app/src/journal.rs`)

#### Scenario: kt-cli and the app agree on the lock directory
- **WHEN** the app takes the lock of a project, and `kt-cli` checks the same project
- **THEN** `kt-cli` reports the holder's process, i.e. both use `kinetren/locks`
- **TEST** t831 (`src/apps/kt-cli/tests/cli.rs`)
